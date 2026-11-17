import { answerHostKey, hostKeyQuestions, onHostKeyAsked, onHostKeyWithdrawn, type HostKeyAsk } from "$lib/ipc";

export function queued(asks: readonly HostKeyAsk[], ask: HostKeyAsk): HostKeyAsk[] {
  return asks.some((known) => known.id === ask.id) ? [...asks] : [...asks, ask];
}

export function withdrawn(asks: readonly HostKeyAsk[], id: number): HostKeyAsk[] {
  return asks.filter((ask) => ask.id !== id);
}

class HostKeyQueue {
  asks = $state.raw<HostKeyAsk[]>([]);
  current = $derived<HostKeyAsk | null>(this.asks[0] ?? null);

  add(ask: HostKeyAsk): void {
    this.asks = queued(this.asks, ask);
  }

  withdraw(id: number): void {
    this.asks = withdrawn(this.asks, id);
  }

  answer(id: number, accept: boolean): void {
    if (!this.asks.some((ask) => ask.id === id)) return;
    this.withdraw(id);
    void answerHostKey(id, accept);
  }

  listen(): () => void {
    const stops = [onHostKeyAsked((ask) => this.add(ask)), onHostKeyWithdrawn((id) => this.withdraw(id))];
    void hostKeyQuestions().then((pending) => {
      for (const ask of pending) this.add(ask);
    });
    return () => {
      for (const stop of stops) void stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }
}

export const hostKeys = new HostKeyQueue();
