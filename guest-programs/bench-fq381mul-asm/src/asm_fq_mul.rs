// Placeholder: ark-ff fallback only. The riscv64 assembly kernel lands here.
use ark_bls12_381::Fq;

pub fn mul_assign(a: &mut Fq, b: &Fq) {
    *a *= b;
}
