import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Attachment } from "svelte/attachments";
import { onMenuAction } from "$lib/child-window";
import { onMenuCommand, popupContextMenu, spellingMisspelled, spellingSuggestions, type SpellLanguage } from "$lib/ipc";
import { SPELL_ADD, SPELL_USE, accepted, mirrorHtml, spellLanguages, spellMenu, spellWords } from "$lib/spelling";
import { notices } from "$stores/notices.svelte";
import { settings } from "$stores/settings.svelte";

const SPELL_PAUSE_MS = 250;
const MAIN_WINDOW = "main";
const COPIED = [
  "boxSizing",
  "fontFamily",
  "fontSize",
  "fontWeight",
  "lineHeight",
  "letterSpacing",
  "tabSize",
  "paddingTop",
  "paddingLeft",
  "paddingBottom",
  "borderTopWidth",
  "borderRightWidth",
  "borderBottomWidth",
  "borderLeftWidth",
  "borderTopStyle",
  "borderRightStyle",
  "borderBottomStyle",
  "borderLeftStyle",
] as const;

const spelled = new Map<string, boolean>();
let spelledFor = "";
let menuOwner: object | null = null;

export function spellField(text: () => string): Attachment<HTMLTextAreaElement | HTMLInputElement> {
  return (field) => {
    const oneLine = field instanceof HTMLInputElement;
    const parent = field.parentElement;
    if (!parent) return;
    if (getComputedStyle(parent).position === "static") parent.style.position = "relative";
    const mirror = document.createElement("div");
    mirror.className = "spell-mirror";
    mirror.setAttribute("aria-hidden", "true");
    Object.assign(mirror.style, {
      position: "absolute",
      overflow: "hidden",
      color: "transparent",
      background: "transparent",
      borderColor: "transparent",
      whiteSpace: oneLine ? "pre" : "pre-wrap",
      overflowWrap: oneLine ? "normal" : "break-word",
      pointerEvents: "none",
      zIndex: "1",
    });
    parent.insertBefore(mirror, field.nextSibling);

    let misses = $state.raw<ReadonlySet<string>>(new Set());
    let run = 0;
    let open: { start: number; word: string; suggestions: string[] } | null = null;
    const label = getCurrentWindow().label;
    const inMain = label === MAIN_WINDOW;

    function place() {
      const style = getComputedStyle(field);
      for (const key of COPIED) mirror.style[key] = style[key];
      const scrollbar = field.offsetWidth - field.clientWidth - parseFloat(style.borderLeftWidth) - parseFloat(style.borderRightWidth);
      mirror.style.paddingRight = `${parseFloat(style.paddingRight) + scrollbar}px`;
      mirror.style.left = `${field.offsetLeft}px`;
      mirror.style.top = `${field.offsetTop}px`;
      mirror.style.width = `${field.offsetWidth}px`;
      mirror.style.height = `${field.offsetHeight}px`;
      if (oneLine) mirror.style.lineHeight = `${field.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom)}px`;
      onscroll();
    }

    async function spell(chosen: SpellLanguage[], personal: readonly string[], found: readonly string[]) {
      const mine = ++run;
      const key = chosen.join(",");
      if (key !== spelledFor) {
        spelled.clear();
        spelledFor = key;
      }
      const unknown = [...new Set(found)].filter((word) => !spelled.has(word) && !accepted(word, personal));
      if (chosen.length > 0 && unknown.length > 0) {
        let wrong: Set<string>;
        try {
          wrong = new Set(await spellingMisspelled(chosen, unknown));
        } catch (err) {
          notices.report(err, "Could not check the spelling");
          return;
        }
        if (mine !== run) return;
        for (const word of unknown) spelled.set(word, wrong.has(word));
      }
      misses = new Set(found.filter((word) => chosen.length > 0 && spelled.get(word) === true && !accepted(word, personal)));
    }

    $effect(() => {
      const chosen = spellLanguages(settings.current);
      const personal = settings.current.spellWords;
      const found = spellWords(text()).map((one) => one.word);
      const timer = setTimeout(() => void spell(chosen, personal, found), SPELL_PAUSE_MS);
      return () => clearTimeout(timer);
    });

    $effect(() => {
      const value = text();
      mirror.innerHTML = mirrorHtml(value, spellWords(value), misses);
      place();
    });

    function replace(start: number, word: string, replacement: string) {
      if (field.value.slice(start, start + word.length) !== word) return;
      field.focus();
      field.setSelectionRange(start, start + word.length);
      if (!document.execCommand("insertText", false, replacement)) {
        field.setRangeText(replacement, start, start + word.length, "end");
        field.dispatchEvent(new Event("input", { bubbles: true }));
      }
    }

    function chosenAction(action: string) {
      const at = open;
      open = null;
      if (!at || menuOwner !== mirror) return;
      if (action === SPELL_ADD) {
        void settings.apply({ ...settings.current, spellWords: [...settings.current.spellWords, at.word] });
      } else if (action.startsWith(SPELL_USE)) {
        const replacement = at.suggestions[Number(action.slice(SPELL_USE.length))];
        if (replacement !== undefined) replace(at.start, at.word, replacement);
      }
    }

    async function onmenu(event: MouseEvent) {
      const hit = [...mirror.querySelectorAll<HTMLElement>(".miss")].find((span) =>
        [...span.getClientRects()].some(
          (box) => event.clientX >= box.left && event.clientX <= box.right && event.clientY >= box.top && event.clientY <= box.bottom,
        ),
      );
      if (!hit) return;
      event.preventDefault();
      const word = hit.textContent ?? "";
      const suggestions = await spellingSuggestions(spellLanguages(settings.current), word).catch((err: unknown) => {
        notices.report(err, "Could not read the spelling suggestions");
        return [];
      });
      open = { start: Number(hit.dataset.at), word, suggestions };
      menuOwner = mirror;
      await popupContextMenu(spellMenu(inMain ? null : label, word, suggestions), event.clientX, event.clientY).catch(
        (err: unknown) => notices.report(err, "Could not show the spelling suggestions"),
      );
    }

    function onscroll() {
      mirror.scrollTop = field.scrollTop;
      mirror.scrollLeft = field.scrollLeft;
    }
    const oncontextmenu = (event: Event) => void onmenu(event as MouseEvent);
    field.addEventListener("scroll", onscroll);
    field.addEventListener("contextmenu", oncontextmenu);
    const watcher = new ResizeObserver(place);
    watcher.observe(field);
    watcher.observe(parent);
    const stopChild = inMain ? () => {} : onMenuAction(window, chosenAction);
    const stopMain = inMain ? onMenuCommand(chosenAction) : null;

    return () => {
      field.removeEventListener("scroll", onscroll);
      field.removeEventListener("contextmenu", oncontextmenu);
      watcher.disconnect();
      stopChild();
      void stopMain?.then((unlisten) => unlisten());
      mirror.remove();
    };
  };
}
