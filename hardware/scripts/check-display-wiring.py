#!/usr/bin/env python3
"""Check expanded display schematics, including every digit/pixel and ERC.

Run make check-display-wiring in hardware/. Uses KiCad 10 and Python stdlib.
This verifies connectivity, not power capacity, timing, or physical part fit.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile

spec = importlib.util.spec_from_file_location('wiring', Path(__file__).with_name('check-wiring.py'))
wiring = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wiring)
Board, require, HARDWARE = wiring.Board, wiring.require, wiring.HARDWARE


def cap(b, ref, rail, value='100nF'):
    b.on(rail, (ref, 1)); b.on('GND', (ref, 2)); b.value(ref, value)


def interface(b, mcu, pins, oled, pullups):
    contract = ['VSYS', 'VSYS', 'GND', 'GND', '+3V3', 'SPI_SCLK',
                'SPI_MOSI', 'SPI_CS', 'DISP_IRQ', 'DISP_NRST', 'DISP_BOOT', 'GND']
    for i, name in enumerate(contract, 1):
        b.on(name, ('J1', i))
    for name, pin in pins.items():
        b.on(name, (mcu, pin))
    for pin, name in enumerate(['+3V3', 'GND', 'OLED_SCL', 'OLED_SDA'], 1):
        b.on(name, (oled, pin))
    for r, name in zip(pullups, ['OLED_SCL', 'OLED_SDA']):
        b.on('+3V3', (r, 1)); b.on(name, (r, 2)); b.value(r, '4.7k')
    for ref in [oled, *pullups]:
        require(any(p.get('name') == 'dnp' for p in b.components[ref].findall('property')),
                f'{ref}: optional OLED and pullups must default to DNP')


def seven_segment(b):
    interface(b, 'U4', dict(SPI_SCLK=12, SPI_MOSI=14, SPI_CS=11, DISP_IRQ=7,
                           DISP_NRST=6, OLED_SCL=30, OLED_SDA=31), 'J2', ['R5', 'R6'])
    b.same(('U4', 25), ('J3', 4), ('R4', 2), exact=True)
    b.same(('U4', 24), ('J3', 2), exact=True)
    b.on('DISP_BOOT', ('R4', 1), ('R1', 1)); b.on('GND', ('R1', 2))
    b.value('R4', '1k'); b.value('R1', '10k'); b.nc('J3', 6)
    b.on('+3V3', ('U4', 4), ('J3', 1)); b.on('GND', ('U4', 5), ('U4', 33), ('J3', 5))
    b.on('DISP_NRST', ('J3', 3), ('C12', 1)); b.on('GND', ('C12', 2))
    for i in range(8, 11): cap(b, f'C{i}', '+3V3')
    cap(b, 'C11', '+3V3', '4.7uF')
    for i in range(1, 4):
        u = f'U{i}'
        cap(b, f'C{i}', '+5V'); cap(b, f'C{i+3}', '+5V', '10uF')
        b.on('+5V', (u, 17)); b.on('GND', (u, 6))
        b.on(f'DIN{i}', (u, 7)); b.on('DISP_CLK', (u, 8))
        digits = [f'DS{(i-1)*16+j}' for j in range(1, 17)]
        # FJ5161AH: A B C D E F G DP -> pins 7 6 4 2 1 9 10 5.
        for seg, pin in enumerate([7, 6, 4, 2, 1, 9, 10, 5], 9):
            b.same((u, seg), *[(r, pin) for r in digits], exact=True)
        for grid, r in enumerate(digits, 1):
            driver_pin = grid + 17 if grid <= 11 else grid - 11
            b.same((u, driver_pin), (r, 3), (r, 8), exact=True)
    for i, pin in enumerate([15, 16, 17, 27], 6):
        require(b.components[f'U{i}'].findtext('footprint') == 'Package_TO_SOT_SMD:SOT-353_SC-70-5',
                f'U{i}: the selected 74HCT1G125GW requires SOT-353')
        b.same(('U4', pin), (f'U{i}', 2), exact=True)
        b.on('+5V', (f'U{i}', 5)); b.on('GND', (f'U{i}', 1), (f'U{i}', 3))
        cap(b, f'C{i+10}', '+5V')
    b.same(('U6', 4), ('U1', 8), ('U2', 8), ('U3', 8), exact=True)
    for i in range(1, 4): b.same((f'U{i+6}', 4), (f'U{i}', 7), exact=True)
    b.on('VSYS', ('U5', 5), ('U5', 7), ('L1', 1))
    b.same(('L1', 2), ('U5', 2), exact=True)
    b.on('+5V', ('U5', 3), ('R2', 1))
    b.same(('U5', 4), ('R2', 2), ('R3', 1), exact=True)
    b.on('GND', ('U5', 1), ('U5', 6), ('R3', 2))
    for ref, rail, value in [('C7','VSYS','10uF'), ('C13','VSYS','10uF'),
                             ('C14','+5V','22uF'), ('C15','+5V','22uF')]: cap(b, ref, rail, value)
    b.distinct(('J1', 1), ('J1', 3), ('J1', 5), ('U5', 3), ('U5', 2), ('U5', 4))


def matrix(b):
    interface(b, 'U1', dict(SPI_SCLK=4, SPI_MOSI=2, SPI_CS=3, DISP_IRQ=38,
                           DISP_NRST=26, OLED_SCL=7, OLED_SDA=6), 'J3', ['R13', 'R14'])
    for i in range(1, 2305):
        b.on('VLED', (f'D{i}', 3)); b.on('GND', (f'D{i}', 1))
    for chain in range(3):
        first = chain * 768 + 1; last = first + 767; u = f'U{chain+4}'
        b.same(('U1', 27+chain), (u, 2), exact=True)
        b.same((u, 4), (f'D{first}', 2), exact=True)
        for i in range(first, last): b.same((f'D{i}', 4), (f'D{i+1}', 2), exact=True)
        b.same((f'D{last}', 4), exact=True)  # exposed, intentionally unused chain end
        b.on('VLED', (u, 5)); b.on('GND', (u, 1), (u, 3))
    for i in range(1001, 1289): cap(b, f'C{i}', 'VLED')
    for i in [15, 16, 17, 18]: cap(b, f'C{i}', 'VLED')
    for i in [13, 14]: cap(b, f'C{i}', 'VLED', '22uF')
    b.on('LED_VSYS', ('J2', 1), ('Q1', 2), ('R5', 1))
    b.on('VLED', ('Q1', 3)); b.on('GND', ('J2', 2), ('Q2', 2), ('R7', 2))
    b.same(('Q1', 1), ('Q2', 3), ('R5', 2), exact=True)
    b.same(('Q2', 1), ('R6', 2), ('R7', 1), exact=True)
    b.same(('U1', 34), ('R6', 1), exact=True)
    b.distinct(('J1', 1), ('J2', 1), ('Q1', 3), ('J1', 3), ('J1', 5), ('U1', 45))
    for pin in [1, 10, 22, 33, 42, 43, 44, 48, 49]: b.on('+3V3', ('U1', pin))
    for pin in [23, 45, 50]: b.on('DVDD', ('U1', pin))
    b.on('GND', ('U1', 19), ('U1', 57))
    for i in [1, 2, 3, 4, 5, 6, 7, 22, 23]: cap(b, f'C{i}', '+3V3')
    for i in [20, 21]: cap(b, f'C{i}', 'DVDD')
    cap(b, 'C8', 'DVDD', '1uF'); cap(b, 'C19', '+3V3', '1uF')
    for i in [9, 10]: cap(b, f'C{i}', '+3V3', '10uF')
    for upin, fpin in [(56,1), (52,6), (53,5), (55,2), (54,3), (51,7)]:
        b.same(('U1', upin), ('U2', fpin))
    b.same(('U1', 56), ('U2', 1), ('R4', 2), ('R10', 1), ('R11', 1), exact=True)
    b.on('+3V3', ('U2', 8), ('R4', 1), ('R12', 1), ('J5', 1))
    b.on('GND', ('U2', 4), ('SW1', 2), ('J5', 5))
    b.same(('R10', 2), ('SW1', 1), exact=True); b.same(('R11', 2), ('J1', 11), exact=True)
    b.value('R10', '1k'); b.value('R11', '1k'); b.value('R12', '10k')
    for upin, jpin in [(24,4), (25,2), (26,3)]: b.same(('U1', upin), ('J5', jpin))
    b.on('DISP_NRST', ('R12', 2)); b.nc('J5', 6)
    b.same(('U1', 20), ('Y1', 1), ('C11', 1), exact=True)
    b.same(('Y1', 3), ('R1', 1), ('C12', 1), exact=True)
    b.same(('U1', 21), ('R1', 2), exact=True)
    b.on('GND', ('Y1', 2), ('Y1', 4), ('C11', 2), ('C12', 2))
    b.value('C11', '15pF'); b.value('C12', '15pF'); b.value('R1', '1k')
    for pin, r in [('A5','R8'), ('B5','R9')]:
        b.same(('J4', pin), (r, 1), exact=True); b.on('GND', (r, 2)); b.value(r, '5.1k')
    for upin, r, jpins, epins in [(47,'R2',['A6','B6'],[1,6]), (46,'R3',['A7','B7'],[3,4])]:
        b.same(('U1', upin), (r, 1), exact=True); b.value(r, '27')
        b.same((r, 2), *[('J4', p) for p in jpins], *[('U3', p) for p in epins], exact=True)
    b.on('VBUS', ('U3', 5), *[('J4', p) for p in ['A4','A9','B4','B9']])
    b.on('GND', ('U3', 2), *[('J4', p) for p in ['A1','A12','B1','B12','SH']])
    b.nc('J4', 'A8'); b.nc('J4', 'B8')


def run(out):
    out.mkdir(parents=True, exist_ok=True)
    cli = os.environ.get('KICAD_CLI', 'kicad-cli')
    for name, count, check in [('display',86,seven_segment), ('matrix',2644,matrix)]:
        project = f'calcumaker-{name}'; sch = HARDWARE/project/(project+'.kicad_sch')
        xml = out/(name+'.xml'); erc = out/(name+'-erc.json')
        for args in [('export','netlist','--format','kicadxml','-o',xml), ('erc','--format','json','-o',erc)]:
            result = subprocess.run([cli, 'sch', *map(str,args), str(sch)], capture_output=True, text=True, check=True)
            require('annotation errors' not in result.stderr + result.stdout, f'{name}: annotation errors')
        violations = [(s['path'],v) for s in json.loads(erc.read_text())['sheets'] for v in s['violations']]
        reviewed = {'3fe1b076-306b-4fdf-baf2-65abe3593a4e', '5a41aa7d-469b-421e-b887-8e8c1bbcceea',
                    '6e0de99a-193a-4c2b-8dff-3448f797af01', '6b83515b-762e-405b-b37d-cda31df67714'}
        accepted = [(p,v) for p,v in violations if name == 'matrix' and v['severity'] == 'warning'
                    and v['type'] == 'isolated_pin_label' and all(i['uuid'] in reviewed for i in v['items'])]
        require(len(accepted) == len(violations), f'{name}: unreviewed ERC findings: {violations}')
        check(Board(xml, count))
        print(f'{project}: {count} components; {len(accepted)} reviewed chain-end warnings; no ERC errors')
    print('PASS: 48 digit mappings, all 2304 pixels / 3 chains, 288 bypasses, rails, interfaces, USB, boot and debug')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path)
    args = parser.parse_args()
    try:
        if args.output_dir: run(args.output_dir)
        else:
            with tempfile.TemporaryDirectory(prefix='calcumaker-displays-') as temp: run(Path(temp))
    except (ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'DISPLAY WIRING CHECK FAILED: {error}\n')
