import { notices } from "$stores/notices.svelte";

export const errors = {
  report(error: unknown, title: string): void {
    notices.report(error, title);
  },

  message(text: string, title: string): void {
    notices.message(text, title);
  },
};
