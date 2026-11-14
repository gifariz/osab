/** Short number for labels and tables: 4 significant digits, "0" for round-off next to `ref`. */
export function fmt(v: number, ref = 0, digits = 4): string {
  if (!Number.isFinite(v)) return String(v);
  if (v === 0 || Math.abs(v) <= 1e-9 * Math.abs(ref)) return "0";
  const a = Math.abs(v);
  if (a >= 1e6 || a < 1e-3) return v.toExponential(digits - 1).replace(/\.?0+e/, "e");
  return String(Number(v.toPrecision(digits)));
}

/** "UX 30000, RY 25000": the non-zero spring stiffness values of a joint (ux … rz order); `num` formats a value (index 0-2 translations, 3-5 rotations). */
export function springText(k: number[], num: (v: number, i: number) => string = (v) => fmt(v)): string {
  const names = ["UX", "UY", "UZ", "RX", "RY", "RZ"];
  return names
    .map((d, i) => (k[i] ? `${d} ${num(k[i], i)}` : ""))
    .filter(Boolean)
    .join(", ");
}

/** Number for an input box: full precision but no binary noise. */
export function inputNumber(v: number | undefined): string {
  if (v === undefined || !Number.isFinite(v)) return "";
  return String(Number(v.toPrecision(12)));
}
