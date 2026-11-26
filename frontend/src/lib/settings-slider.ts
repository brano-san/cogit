export function readSliderNumber(text: string, min: number, max: number): number | null {
  const integer = text.trim();
  if (!/^[+-]?\d+$/.test(integer)) return null;
  return Math.max(min, Math.min(max, Number(integer)));
}
