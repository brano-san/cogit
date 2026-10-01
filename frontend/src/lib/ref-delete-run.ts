import type { RefDeletion, RefDeletionReport } from "$lib/ipc";

/** What running a deletion needs from the app: the batch call, the error window, and the
    question about branches git calls not fully merged. */
export interface DeleteHost {
  remove: (request: RefDeletion) => Promise<RefDeletionReport>;
  fail: (error: unknown, title: string) => void;
  askForce: (kept: readonly string[]) => Promise<boolean>;
}

export interface DeleteResult {
  deleted: number;
  failed: string[];
}

/** One batch of refs of one kind: whatever git refuses is reported one by one, and the
    branches it calls not fully merged are offered for a forced second pass. */
export async function runDeletion(request: RefDeletion, host: DeleteHost): Promise<DeleteResult> {
  const result: DeleteResult = { deleted: 0, failed: [] };
  const pass = async (again: RefDeletion): Promise<string[]> => {
    try {
      const report = await host.remove(again);
      result.deleted += report.deleted.length;
      for (const each of report.failed) {
        host.fail(each.error, `Could not delete ${each.name}`);
        result.failed.push(each.name);
      }
      return report.notFullyMerged;
    } catch (err) {
      host.fail(err, "Could not delete the refs");
      return [];
    }
  };
  const kept = await pass(request);
  if (kept.length > 0 && (await host.askForce(kept))) await pass({ ...request, names: kept, force: true });
  return result;
}
