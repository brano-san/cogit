import { listCredentials, type SignIn } from "$lib/ipc";
import { signedIn as retried, type SignInRequest } from "$lib/sign-in";

interface Pending extends SignInRequest {
  /** Preferences ▸ Authentication ▸ Add…: the host is typed and the sign-in is kept as is. */
  adding: boolean;
  settle: (answer: SignIn | null) => void;
}

/** The Sign in dialog, awaited like `confirmation.ask` (F-082). */
class SignInStore {
  open = $state.raw<Pending | null>(null);
  /** No OS keychain: Remember keeps a sign-in until Cogit exits. */
  persistent = $state(true);

  ask(request: SignInRequest, adding = false): Promise<SignIn | null> {
    this.open?.settle(null);
    listCredentials()
      .then((list) => (this.persistent = list.persistent))
      .catch((err: unknown) => console.error("could not read the sign-ins", err));
    return new Promise((resolve) => {
      this.open = {
        ...request,
        adding,
        settle: (answer) => {
          this.open = null;
          resolve(answer);
        },
      };
    });
  }

  answer(value: SignIn | null): void {
    this.open?.settle(value);
  }
}

export const signIn = new SignInStore();

/** `operation`, run again with what the Sign in dialog was given while the remote refuses. */
export function signedIn<T>(operation: (credential: SignIn | null) => Promise<T>): Promise<T> {
  return retried(operation, (request) => signIn.ask(request));
}
