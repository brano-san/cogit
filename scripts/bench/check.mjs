/** What `--check` reports: one line per budget the run did not meet. A scenario that only
    failed, failed some of the time, or never ran meets nothing — `expected` says which
    budgets this run was asked to measure (`--sets`, `--only`, `--no-*`). */
export function overBudget(rows, budgets, expected) {
  const over = [];
  for (const budget of budgets) {
    if (!expected(budget)) continue;
    const name = `${budget.id} · ${budget.set} · ${budget.condition}`;
    const row = rows.find((r) => r.id === budget.id && r.set === budget.set && r.condition === budget.condition);
    if (!row) over.push(`${name}: not measured`);
    else if (row.errors.length > 0) over.push(`${name}: ${row.errors.length} errors, first: ${row.errors[0]}`);
    else if (row.median === null) over.push(`${name}: no sample`);
    else if (row.median > budget.median) over.push(`${name}: ${row.median} ms > ${budget.median} ms`);
  }
  return over;
}
