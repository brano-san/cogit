import { textX } from "$lib/graph-geometry";

/** The narrowest the Graph panel goes, splitter or window alike: one lane and about this
    many characters of the subject. Nothing inside it hides a column (#5, R-243). */
export const GRAPH_MIN_SUBJECT_CHARS = 35;

/** A CSS length, so the characters are measured in the font the rows are drawn in. */
export function graphPanelMinWidth(): string {
  return `calc(${textX(1)}px + ${GRAPH_MIN_SUBJECT_CHARS}ch)`;
}

export function subjectMinWidth(): string {
  return `${GRAPH_MIN_SUBJECT_CHARS}ch`;
}
