import type { SignIn } from "$lib/ipc";
import { signedIn as retried, type SignInRequest } from "$lib/sign-in";
import { masterPassword } from "$stores/master-password.svelte";
import { notices } from "$stores/notices.svelte";

interface Pending extends SignInRequest {
  /** Preferences ▸ Authentication ▸ Add…: the host is typed and the sign-in is kept as is. */
  adding: boolean;
  settle: (answer: SignIn | null) => void;
}

/** The Sign in dialog, awaited like `confirmation.ask` (F-082). */
class SignInStore {
  open = $state.raw<Pending | null>(null);

  ask(request: SignInRequest, adding = false): Promise<SignIn | null> {
    this.open?.settle(null);
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

/** A store that cannot be opened is no reason to fail the command: it runs without it. */
async function settled(step: Promise<boolean>): Promise<boolean> {
  try {
    return await step;
  } catch (err) {
    notices.report(err, "Could not open the password store");
    return false;
  }
}

/** `operation`, run again with what the Sign in dialog was given while the remote refuses.
    The master password is asked first, once a session (R-891); Remember needs an open store. */
export async function signedIn<T>(operation: (credential: SignIn | null) => Promise<T>): Promise<T> {
  await settled(masterPassword.beforeUse().then(() => true));
  return retried(operation, async (request) => {
    const answer = await signIn.ask(request);
    if (answer?.remember && !(await settled(masterPassword.ready()))) return { ...answer, remember: false };
    return answer;
  });
}
