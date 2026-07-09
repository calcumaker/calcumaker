//! Dense arbitrary-precision matrices (MPFR `Float` elements) — the HP-15C
//! matrix feature, modernized onto the stack (`Value::Matrix`) rather than the
//! 15C's A–E named registers. Row-major storage; the linear algebra
//! (determinant / inverse / solve) is one LU decomposition with partial
//! pivoting, done at the working precision.

use alloc::vec::Vec;
use gmp_mpfr_nostd::Float;

/// A `rows × cols` matrix of `Float`s at a fixed working precision.
#[derive(Clone)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    prec: u32,
    data: Vec<Float>, // row-major, rows*cols elements
}

/// `|a| > |b|` without a dedicated FFI compare — sign of `|a| − |b|`.
fn abs_gt(a: &Float, b: &Float) -> bool {
    let d = a.clone().abs() - b.clone().abs();
    !d.is_sign_negative() && !d.is_zero()
}

impl Matrix {
    /// A zero-filled `rows × cols` matrix.
    pub fn zeros(rows: usize, cols: usize, prec: u32) -> Self {
        Matrix {
            rows,
            cols,
            prec,
            data: (0..rows * cols).map(|_| Float::from_i64(prec, 0)).collect(),
        }
    }

    /// Build from equal-length rows; `None` on a ragged input or no rows.
    pub fn from_rows(prec: u32, rows: &[Vec<Float>]) -> Option<Matrix> {
        let r = rows.len();
        if r == 0 {
            return None;
        }
        let c = rows[0].len();
        if c == 0 || rows.iter().any(|row| row.len() != c) {
            return None;
        }
        let mut m = Matrix::zeros(r, c, prec);
        for (i, row) in rows.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                m.set(i, j, Float::with_prec(prec, v));
            }
        }
        Some(m)
    }

    /// The `n × n` identity.
    pub fn identity(n: usize, prec: u32) -> Matrix {
        let mut m = Matrix::zeros(n, n, prec);
        for i in 0..n {
            m.set(i, i, Float::from_i64(prec, 1));
        }
        m
    }

    pub fn rows(&self) -> usize {
        self.rows
    }
    pub fn cols(&self) -> usize {
        self.cols
    }
    pub fn prec(&self) -> u32 {
        self.prec
    }
    pub fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    #[inline]
    fn idx(&self, i: usize, j: usize) -> usize {
        i * self.cols + j
    }
    pub fn get(&self, i: usize, j: usize) -> &Float {
        &self.data[self.idx(i, j)]
    }
    pub fn set(&mut self, i: usize, j: usize, v: Float) {
        let k = self.idx(i, j);
        self.data[k] = v;
    }

    fn swap_rows(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        for j in 0..self.cols {
            let (ka, kb) = (a * self.cols + j, b * self.cols + j);
            self.data.swap(ka, kb);
        }
    }

    pub fn transpose(&self) -> Matrix {
        let mut t = Matrix::zeros(self.cols, self.rows, self.prec);
        for i in 0..self.rows {
            for j in 0..self.cols {
                t.set(j, i, self.get(i, j).clone());
            }
        }
        t
    }

    /// Element-wise `self ± other` (`None` on a shape mismatch).
    pub fn add(&self, o: &Matrix) -> Option<Matrix> {
        self.zip(o, |a, b| a + b)
    }
    pub fn sub(&self, o: &Matrix) -> Option<Matrix> {
        self.zip(o, |a, b| a - b)
    }
    fn zip(&self, o: &Matrix, f: impl Fn(Float, Float) -> Float) -> Option<Matrix> {
        if self.rows != o.rows || self.cols != o.cols {
            return None;
        }
        let mut m = Matrix::zeros(self.rows, self.cols, self.prec);
        for i in 0..self.data.len() {
            m.data[i] = f(self.data[i].clone(), o.data[i].clone());
        }
        Some(m)
    }

    /// Scale every element by `s`.
    pub fn scalar_mul(&self, s: &Float) -> Matrix {
        let mut m = Matrix::zeros(self.rows, self.cols, self.prec);
        for i in 0..self.data.len() {
            m.data[i] = self.data[i].clone() * s.clone();
        }
        m
    }

    /// Matrix product (`None` unless `self.cols == o.rows`).
    pub fn mul(&self, o: &Matrix) -> Option<Matrix> {
        if self.cols != o.rows {
            return None;
        }
        let mut m = Matrix::zeros(self.rows, o.cols, self.prec);
        for i in 0..self.rows {
            for j in 0..o.cols {
                let mut acc = Float::from_i64(self.prec, 0);
                for k in 0..self.cols {
                    acc = acc + self.get(i, k).clone() * o.get(k, j).clone();
                }
                m.set(i, j, acc);
            }
        }
        Some(m)
    }

    /// LU decomposition with partial pivoting (Doolittle; unit lower diagonal
    /// stored below the diagonal, U on/above). Returns `(LU, pivots, sign)`, or
    /// `None` if `self` is singular (a zero pivot). Caller checks squareness.
    fn lu(&self) -> Option<(Matrix, Vec<usize>, i32)> {
        let n = self.rows;
        let mut a = self.clone();
        let mut piv: Vec<usize> = (0..n).collect();
        let mut sign = 1i32;
        for k in 0..n {
            let mut pivot_row = k;
            for i in (k + 1)..n {
                if abs_gt(a.get(i, k), a.get(pivot_row, k)) {
                    pivot_row = i;
                }
            }
            if a.get(pivot_row, k).is_zero() {
                return None; // singular
            }
            if pivot_row != k {
                a.swap_rows(k, pivot_row);
                piv.swap(k, pivot_row);
                sign = -sign;
            }
            let pivot = a.get(k, k).clone();
            for i in (k + 1)..n {
                let factor = a.get(i, k).clone() / pivot.clone();
                a.set(i, k, factor.clone());
                for j in (k + 1)..n {
                    let v = a.get(i, j).clone() - factor.clone() * a.get(k, j).clone();
                    a.set(i, j, v);
                }
            }
        }
        Some((a, piv, sign))
    }

    /// Determinant of a square matrix (`None` if not square); a singular matrix
    /// gives exactly zero.
    pub fn determinant(&self) -> Option<Float> {
        if !self.is_square() {
            return None;
        }
        match self.lu() {
            None => Some(Float::from_i64(self.prec, 0)),
            Some((lu, _, sign)) => {
                let mut det = Float::from_i64(self.prec, sign as i64);
                for k in 0..self.rows {
                    det = det * lu.get(k, k).clone();
                }
                Some(det)
            }
        }
    }

    /// Solve `A · X = B` for `X` (this = `A`). `None` if `A` isn't square, the
    /// shapes don't line up, or `A` is singular.
    pub fn solve(&self, b: &Matrix) -> Option<Matrix> {
        if !self.is_square() || b.rows != self.rows {
            return None;
        }
        let (lu, piv, _) = self.lu()?;
        let n = self.rows;
        let mut x = Matrix::zeros(n, b.cols, self.prec);
        for col in 0..b.cols {
            // permute the RHS, then forward/back substitute.
            let mut y: Vec<Float> = (0..n).map(|i| b.get(piv[i], col).clone()).collect();
            for i in 0..n {
                for k in 0..i {
                    y[i] = y[i].clone() - lu.get(i, k).clone() * y[k].clone();
                }
            }
            for i in (0..n).rev() {
                for k in (i + 1)..n {
                    y[i] = y[i].clone() - lu.get(i, k).clone() * y[k].clone();
                }
                y[i] = y[i].clone() / lu.get(i, i).clone();
            }
            for i in 0..n {
                x.set(i, col, y[i].clone());
            }
        }
        Some(x)
    }

    /// Inverse via `A · X = I` (`None` if not square or singular).
    pub fn inverse(&self) -> Option<Matrix> {
        if !self.is_square() {
            return None;
        }
        self.solve(&Matrix::identity(self.rows, self.prec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn m(prec: u32, rows: &[&[i64]]) -> Matrix {
        let rr: Vec<Vec<Float>> = rows
            .iter()
            .map(|r| r.iter().map(|&v| Float::from_i64(prec, v)).collect())
            .collect();
        Matrix::from_rows(prec, &rr).unwrap()
    }

    #[test]
    fn determinant_2x2_and_3x3() {
        use gmp_mpfr_nostd::Integer;
        // |1 2; 3 4| = -2 (round: LU introduces 2/3, so compare rounded).
        assert!(
            m(128, &[&[1, 2], &[3, 4]])
                .determinant()
                .unwrap()
                .round_to_int()
                == Integer::from_i64(-2)
        );
        // singular -> exactly 0
        assert!(m(128, &[&[1, 2], &[2, 4]]).determinant().unwrap().is_zero());
        // |2 0 0; 0 3 0; 0 0 4| = 24 (diagonal LU is exact)
        assert_eq!(
            m(128, &[&[2, 0, 0], &[0, 3, 0], &[0, 0, 4]])
                .determinant()
                .unwrap()
                .cmp_si(24),
            0
        );
    }

    #[test]
    fn solve_and_inverse_roundtrip() {
        // A x = b : [[2,1],[1,3]] x = [3,5]^T  -> x = [0.8, 1.4]
        let a = m(128, &[&[2, 1], &[1, 3]]);
        let b = Matrix::from_rows(
            128,
            &[vec![Float::from_i64(128, 3)], vec![Float::from_i64(128, 5)]],
        )
        .unwrap();
        let x = a.solve(&b).unwrap();
        // A * x should equal b
        let check = a.mul(&x).unwrap();
        assert!(check.get(0, 0).clone().equals(b.get(0, 0)));
        assert!(check.get(1, 0).clone().equals(b.get(1, 0)));
        // A * A^-1 = I
        let inv = a.inverse().unwrap();
        let id = a.mul(&inv).unwrap();
        assert_eq!(id.get(0, 0).cmp_si(1), 0);
        assert!(id.get(0, 1).is_zero());
        assert!(id.get(1, 0).is_zero());
        assert_eq!(id.get(1, 1).cmp_si(1), 0);
    }

    #[test]
    fn transpose_and_mul() {
        let a = m(64, &[&[1, 2, 3], &[4, 5, 6]]); // 2x3
        let t = a.transpose(); // 3x2
        assert_eq!((t.rows(), t.cols()), (3, 2));
        assert_eq!(t.get(2, 1).cmp_si(6), 0);
        // (2x3)(3x2) = 2x2
        let p = a.mul(&t).unwrap();
        assert_eq!((p.rows(), p.cols()), (2, 2));
        // [1,2,3]·[1,2,3] = 14
        assert_eq!(p.get(0, 0).cmp_si(14), 0);
    }
}
