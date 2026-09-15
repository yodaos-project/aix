import ora from "ora";

export async function withLoading<T>(
  message: string,
  task: () => Promise<T>,
): Promise<T> {
  if (!process.stderr.isTTY) {
    return await task();
  }

  const spinner = ora({
    text: message,
    stream: process.stderr,
  });

  spinner.start();
  try {
    const result = await task();
    spinner.stop();
    return result;
  } catch (error) {
    spinner.stop();
    throw error;
  }
}

export function formatError(message: string): string {
  if (!process.stderr.isTTY) {
    return `✖ ${message}`;
  }
  return `\x1b[1;31m✖\x1b[0m \x1b[31m${message}\x1b[0m`;
}
