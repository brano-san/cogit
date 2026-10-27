let held = false;
let waiting: (() => void)[] = [];

function release() {
  held = false;
  const settled = waiting;
  waiting = [];
  for (const settle of settled) settle();
}

if (typeof window !== "undefined") {
  window.addEventListener("pointerdown", (event) => event.button === 2 && (held = true), true);
  window.addEventListener("pointerup", (event) => event.button === 2 && release(), true);
  window.addEventListener("blur", release);
}

export function secondaryReleased(): Promise<void> {
  return held ? new Promise((settle) => waiting.push(settle)) : Promise.resolve();
}
