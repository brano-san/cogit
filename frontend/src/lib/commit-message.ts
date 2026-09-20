export const SUBJECT_SOFT = 50;
export const SUBJECT_HARD = 72;

export type SubjectState = "ok" | "long" | "too-long";

export function subjectOf(message: string): string {
  return message.split("\n", 1)[0] ?? "";
}

export function subjectState(message: string): SubjectState {
  const length = [...subjectOf(message)].length;
  if (length > SUBJECT_HARD) return "too-long";
  if (length > SUBJECT_SOFT) return "long";
  return "ok";
}
