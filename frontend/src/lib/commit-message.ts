export const SUBJECT_SOFT = 50;
export const SUBJECT_HARD = 72;
export const MAX_GUIDE_COLUMN = 300;

export function validGuideColumn(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0 && value <= MAX_GUIDE_COLUMN;
}

export function messageGuides(message: string, enabled: boolean, subject: number, body: number) {
  if (!enabled) return null;
  return { position: `${subject}ch 0, ${body}ch 0`, subjectOver: subjectLength(message) > subject };
}

export type SubjectState = "ok" | "long" | "too-long";

export function subjectOf(message: string): string {
  return message.split(/\r?\n/, 1)[0] ?? "";
}

export function subjectLength(message: string): number {
  return [...subjectOf(message)].length;
}

export function subjectState(message: string): SubjectState {
  const length = subjectLength(message);
  if (length > SUBJECT_HARD) return "too-long";
  if (length > SUBJECT_SOFT) return "long";
  return "ok";
}
