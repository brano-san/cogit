/** Line keys (`d:` old side, `i:` new side) as the line numbers of each side. */
export function splitSelection(selected: ReadonlySet<string>): {
  deletes: number[];
  inserts: number[];
} {
  const deletes: number[] = [];
  const inserts: number[] = [];
  for (const key of selected) {
    const line = Number(key.slice(2));
    if (key.startsWith("d:")) deletes.push(line);
    else inserts.push(line);
  }
  return { deletes, inserts };
}
