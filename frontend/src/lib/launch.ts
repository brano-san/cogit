import { droppedRepositories, openDropped, type DropHost } from "$lib/drop-open";
import type { LaunchRequest } from "$lib/ipc";

/** `cogit <path>…` opens like folders dropped on the window: a file stands for its folder. */
export async function runLaunch(request: LaunchRequest, host: DropHost): Promise<void> {
  await openDropped(droppedRepositories(request.paths.map((each) => each.folder)), host);
}
