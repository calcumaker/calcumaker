#!/usr/bin/env python3
"""Verify the expanded mainboard/keypad netlists, independently of schematic layout.

Run with `make check-wiring` in hardware/. Requires KiCad 10 and Python's stdlib.
Reports and netlists are temporary unless --output-dir is supplied.
"""

import argparse
from collections import Counter
import json
import os
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET

HARDWARE = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


class Board:
    def __init__(self, path, count):
        root = ET.parse(path).getroot()
        components = root.findall('components/comp')
        refs = Counter(c.attrib['ref'] for c in components)
        require(len(components) == count, f'{path}: expected {count} components')
        require(all(n == 1 for n in refs.values()), f'{path}: duplicate references: {refs}')
        self.components = {c.attrib['ref']: c for c in components}
        for ref, c in self.components.items():
            require(c.findtext('footprint'), f'{ref}: missing footprint')
        self.nets = {}
        self.pins = {}
        for net in root.findall('nets/net'):
            name = net.attrib['name']
            members = {(n.attrib['ref'], n.attrib['pin']) for n in net}
            self.nets[name] = members
            for pin in members:
                require(pin not in self.pins, f'{pin}: appears on multiple nets')
                self.pins[pin] = name

    def net(self, ref, pin):
        return self.pins[(ref, str(pin))]

    def on(self, name, *pins):
        for ref, pin in pins:
            require(self.net(ref, pin).split('/')[-1] == name,
                    f'{ref}.{pin}: expected {name}, got {self.net(ref, pin)}')

    def same(self, *pins, exact=False):
        pins = {(r, str(p)) for r, p in pins}
        names = {self.net(*p) for p in pins}
        require(len(names) == 1, f'Expected connected: {pins}; got {names}')
        if exact:
            require(self.nets[next(iter(names))] == pins,
                    f'Unexpected branch on {names}: {self.nets[next(iter(names))] ^ pins}')

    def value(self, ref, prefix):
        value = self.components[ref].findtext('value')
        require(value.startswith(prefix), f'{ref}: expected {prefix}, got {value}')

    def distinct(self, *pins):
        require(len({self.net(*p) for p in pins}) == len(pins), f'Shorted nets: {pins}')

    def nc(self, ref, pin):
        require(self.net(ref, pin).startswith('unconnected-'), f'{ref}.{pin}: expected NC')
        self.same((ref, pin), exact=True)


def check_interfaces(m, k):
    common = ['+3V3', 'GND', 'I2C_SDA', 'I2C_SCL', 'UART_TX', 'UART_RX',
              'KB_IRQ', 'KB_NRST', 'KB_BOOT0', 'GND', 'VSYS']
    for board, stack, ffc in [(m, 'J5', 'J6'), (k, 'J1', 'J3')]:
        for pin, name in enumerate(common, 1):
            board.on(name, (stack, pin), (ffc, pin))
        board.on('GND', (stack, 12), (ffc, 13), (ffc, 14))
        board.on('VSYS', (ffc, 12))
        for pin in [15, 16]:
            board.nc(ffc, pin)
    # UART net names use the mainboard's perspective.
    m.on('UART_TX', ('U1', 16)); k.on('UART_TX', ('U1', 10))
    m.on('UART_RX', ('U1', 17)); k.on('UART_RX', ('U1', 9))
    for name, mpin, kpin in [('I2C_SCL', 58, 30), ('I2C_SDA', 59, 31),
                            ('KB_IRQ', 14, 20), ('KB_NRST', 51, 6)]:
        m.on(name, ('U1', mpin)); k.on(name, ('U1', kpin))
    m.on('KB_BOOT0', ('U1', 52)); k.on('KB_BOOT0', ('R11', 1))


def check_keypad(k):
    keys = [i for i in range(1, 51) if i != 36]
    require({r for r in k.components if r.startswith('SW')} == {f'SW{i}' for i in keys},
            'Matrix must contain exactly 49 switches, with SW36 absent')
    rows = [18, 19, 21, 22, 23]
    cols = [7, 8, 17, 27, 11, 12, 13, 14, 2, 3]
    for row, pin in enumerate(rows):
        members = [('U1', pin)] + [(f'SW{i}', 1) for i in keys if (i-1)//10 == row]
        k.same(*members, exact=True)
    for col, pin in enumerate(cols):
        members = [('U1', pin)] + [(f'D{i}', 2) for i in keys if (i-1)%10 == col]
        k.same(*members, exact=True)
    for i in keys:
        k.same((f'SW{i}', 2), (f'D{i}', 1), exact=True)  # cathode toward selected LOW row
        k.on('VLED', (f'D{i+55}', 3), (f'C{i+100}', 1))
        k.on('GND', (f'D{i+55}', 1), (f'C{i+100}', 2))
        k.value(f'C{i+100}', '100nF')
    chain = [f'D{i+55}' for i in keys]
    k.same(('R9', 2), (chain[0], 2), exact=True)
    for a, b in zip(chain, chain[1:]):
        k.same((a, 4), (b, 2), exact=True)
    # The unused final DOUT is marked NC at the parent sheet pin, so it retains a net name.
    k.same((chain[-1], 4), exact=True)
    for i, pin in enumerate([15, 16, 28, 32, 1], 1):
        k.same(('U1', pin), (f'R{i}', 1), exact=True)
        k.same((f'R{i}', 2), (f'D{i+50}', 2), exact=True)
        k.on('GND', (f'D{i+50}', 1)); k.value(f'R{i}', '470')
    k.same(('U1', 24), ('J2', 2), exact=True)
    k.same(('U1', 25), ('J2', 4), ('R11', 2), ('R6', 1), exact=True)
    k.value('R11', '1k'); k.value('R6', '10k'); k.nc('J2', 6)
    k.same(('U1', 26), ('U2', 2), ('R12', 1), exact=True)
    k.same(('U1', 29), ('R8', 1), exact=True)
    k.same(('R8', 2), ('Q2', 1), ('R10', 1), exact=True)
    k.same(('Q1', 1), ('Q2', 3), ('R7', 2), exact=True)
    k.same(('U2', 4), ('R9', 1), exact=True)
    k.on('VSYS', ('Q1', 2), ('R7', 1))
    k.on('VLED', ('Q1', 3), ('U2', 5), ('C6', 1), ('C7', 1))
    k.on('+3V3', ('U1', 4), ('J2', 1))
    k.on('GND', ('U1', 5), ('U1', 33), ('Q2', 2), ('U2', 1), ('U2', 3),
         ('R6', 2), ('R10', 2), ('R12', 2))
    k.distinct(('J1', 1), ('J1', 2), ('Q1', 2), ('Q1', 3), ('R9', 2))
    lib = k.components['U1'].find('libsource')
    require(lib.attrib == {'lib': 'calcumaker', 'part': 'STM32G031K8U6',
                          'description': lib.attrib.get('description', '')},
            'G031 must use the package-specific symbol with dedicated PA9/PA10 pads')


def check_mainboard(m):
    for upin, fpin in [(29, 6), (20, 1), (27, 5), (26, 2), (23, 3), (22, 7)]:
        m.same(('U1', upin), ('U7', fpin))
    m.same(('U7', 1), ('R9', 2)); m.on('+3V3', ('R9', 1), ('U7', 8), ('C26', 1))
    for upin, jpin in [(21, 6), (57, 7), (41, 8), (33, 9), (39, 10), (40, 11)]:
        m.same(('U1', upin), ('J3', jpin), exact=True)
    m.on('VSYS', ('J3', 1), ('J3', 2), ('J7', 1))
    m.on('GND', ('J3', 3), ('J3', 4), ('J3', 12), ('J7', 2))
    m.on('+3V3', ('J3', 5), ('J4', 1)); m.on('GND', ('J4', 5))
    for upin, jpin in [(46, 2), (49, 4), (55, 6), (7, 3)]:
        m.same(('U1', upin), ('J4', jpin))
    m.same(('U1', 60), ('R8', 1), exact=True)
    m.same(('U1', 30), ('C22', 1), ('C23', 1), exact=True)
    for ref in ['C22', 'C23']:
        m.value(ref, '2.2uF 10V'); m.on('GND', (ref, 2))
    for pin in [1, 13, 19, 32, 48, 64]: m.on('+3V3', ('U1', pin))
    for pin in [12, 18, 31, 47, 63]: m.on('GND', ('U1', pin))
    for pin, cap, ypin in [(3, 'C24', 1), (4, 'C25', 2)]:
        m.same(('U1', pin), (cap, 1), ('Y1', ypin), exact=True)
    m.same(('U1', 50), ('U5', 3), exact=True)
    m.same(('U1', 36), ('U5', 1), exact=True)  # PB15 is pad 36, not ground pad 63
    m.same(('U1', 37), ('U5', 12), ('C31', 1), exact=True)
    m.same(('U1', 38), ('U5', 10), exact=True)
    m.same(('U1', 11), ('U5', 11), ('R11', 2), exact=True)
    for pin, esd in [(44, 4), (45, 6)]: m.same(('U1', pin), ('U3', esd), exact=True)
    m.on('+3V3', ('U3', 5)); m.on('GND', ('U3', 2))
    m.same(('J1', 'A6'), ('J1', 'B6'), ('U3', 1), exact=True)
    m.same(('J1', 'A7'), ('J1', 'B7'), ('U3', 3), exact=True)
    m.same(('J1', 'A5'), ('U5', 7), ('C28', 1), exact=True)
    m.same(('J1', 'B5'), ('U5', 9), ('C29', 1), exact=True)
    m.same(('Q1', 1), ('U5', 5), exact=True)
    m.same(('Q1', 3), ('J1', 'A4'), ('J1', 'B4'), ('U5', 8), ('R4', 1), ('R6', 1))
    m.same(('Q1', 2), ('U5', 4), ('U4', 1), ('U4', 24))
    m.same(('U5', 6), ('R6', 2), ('R7', 1), exact=True)
    m.same(('U1', 8), ('R4', 2), ('R10', 1), exact=True)
    m.same(('U1', 9), ('R20', 2), ('R21', 1), ('C33', 1), exact=True)
    for ref in ['R20', 'R21']: m.value(ref, '1M')
    m.same(('U4', 2), ('R1', 2), exact=True)
    m.same(('U4', 22), ('R1', 1), ('R2', 1), ('C11', 1), exact=True)
    m.same(('U4', 11), ('R2', 2), ('R3', 1), ('J8', 1), exact=True)
    m.same(('U4', 12), ('J9', 1), exact=True)
    m.same(('U4', 19), ('U4', 20), ('L2', 1), ('C27', 2), exact=True)
    m.same(('U4', 21), ('C27', 1), exact=True)
    m.on('VSYS', ('L2', 2), ('U4', 15), ('U4', 16), ('U2', 1), ('U2', 10))
    m.on('BAT+', ('U4', 13), ('U4', 14), ('U6', 2), ('U6', 3), ('J2', 1), ('R20', 1))
    for upin, cpin, ref in [(10, 7, 'R15'), (24, 3, 'R16'), (25, 9, 'R17')]:
        m.same(('U1', upin), ('U4', cpin), (ref, 2), exact=True)
        m.on('+3V3', (ref, 1))
    m.value('R17', '100k')
    m.same(('U1', 2), ('U6', 5), ('R12', 2), exact=True)
    m.on('I2C_SCL', ('U4', 5), ('U6', 7), ('R13', 2))
    m.on('I2C_SDA', ('U4', 6), ('U6', 8), ('R14', 2))
    for ref in ['R13', 'R14']: m.on('+3V3', (ref, 1)); m.value(ref, '4.7k')
    m.same(('U2', 9), ('L1', 1), exact=True); m.same(('U2', 7), ('L1', 2), exact=True)
    m.same(('U2', 3), ('R18', 1), exact=True); m.value('R18', '36.5k')
    m.same(('U2', 5), ('R19', 1), exact=True); m.value('R19', '16.2k')
    m.on('+3V3', ('U2', 6))
    m.on('GND', ('U2', 2), ('U2', 4), ('U2', 8), ('U2', 11), ('U4', 25),
         ('U5', 13), ('U6', 1), ('U6', 4), ('U6', 6), ('U6', 9),
         ('R18', 2), ('R19', 2), ('R21', 2), ('J8', 2), ('J9', 2))
    m.distinct(('U1', 1), ('U1', 63), ('U1', 30), ('J2', 1), ('J7', 1),
               ('Q1', 3), ('Q1', 2), ('L2', 1), ('L1', 1), ('L1', 2))


def run(output_dir):
    output_dir.mkdir(parents=True, exist_ok=True)
    cli = os.environ.get('KICAD_CLI', 'kicad-cli')
    boards = {}
    for name, count in [('mcu', 75), ('keyboard', 227)]:
        project = f'calcumaker-{name}'
        sch = HARDWARE / project / f'{project}.kicad_sch'
        xml = output_dir / f'{name}.xml'
        erc = output_dir / f'{name}-erc.json'
        for args in [('export', 'netlist', '--format', 'kicadxml', '-o', xml),
                     ('erc', '--format', 'json', '-o', erc)]:
            result = subprocess.run([cli, 'sch', *map(str, args), str(sch)],
                                    capture_output=True, text=True, check=True)
            require('annotation errors' not in result.stderr + result.stdout,
                    f'{name}: annotation errors; repair before trusting netlist export')
        violations = [(s['path'], v) for s in json.loads(erc.read_text())['sheets']
                      for v in s['violations']]
        # GPIO-controlled TCPP VCC is explicitly driven; KiCad models PC6 as bidirectional.
        accepted = [(p, v) for p, v in violations if name == 'mcu'
                    and v['type'] == 'pin_to_pin' and v['severity'] == 'warning'
                    and {i['uuid'] for i in v['items']} == {
                        '2e47d5cd-d245-4832-9c89-04b03846a4e0',
                        '76066d39-aa76-43f7-a8aa-2bf4d1b9ba80'}]
        require(len(violations) == len(accepted), f'{name}: unreviewed ERC violations: {violations}')
        boards[name] = Board(xml, count)
        print(f'{project}: {count} components; {len(violations)} reviewed ERC warning(s), no errors')
    check_interfaces(boards['mcu'], boards['keyboard'])
    check_mainboard(boards['mcu'])
    check_keypad(boards['keyboard'])
    print('PASS: connector parity, MCU pins, 49-key matrix, 49-LED chain, bypass caps, power and debug nets')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path)
    args = parser.parse_args()
    try:
        if args.output_dir:
            run(args.output_dir)
        else:
            with tempfile.TemporaryDirectory(prefix='calcumaker-wiring-') as temp:
                run(Path(temp))
    except (ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'WIRING CHECK FAILED: {error}\n')
