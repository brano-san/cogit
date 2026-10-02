/** Wall-clock budgets in tests are set for a developer machine; a shared CI runner is several times slower. */
export const BUDGET_SLACK = typeof process !== "undefined" && process.env?.CI ? 6 : 1;

export const budget = (ms: number): number => ms * BUDGET_SLACK;
