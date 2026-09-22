/** What the user opened in a tree, for as long as Cogit runs and no longer: a start
    shows every list folded, and going back to a repository finds it as it was left
    (doc/12-risks.md, R-160). Never written to disk. */
const lists = new Map<string, Map<string, ReadonlySet<string>>>();

export function recall(list: string, place: string): ReadonlySet<string> {
  return new Set(lists.get(list)?.get(place) ?? []);
}

export function remember(list: string, place: string, open: ReadonlySet<string>): void {
  let places = lists.get(list);
  if (!places) {
    places = new Map();
    lists.set(list, places);
  }
  places.set(place, new Set(open));
}
