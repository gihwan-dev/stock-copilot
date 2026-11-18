import { mkdtempSync as create, rmSync } from "node:fs";

const directories = new Set();

export const cleanup = () => {
  for (const directory of directories) {
    rmSync(directory, { recursive: true, force: true });
    directories.delete(directory);
  }
};

export const mkdtempSync = (...args) => {
  const directory = create(...args);
  directories.add(directory);
  return directory;
};

process.once("exit", cleanup);
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  const terminate = () => {
    cleanup();
    process.removeListener(signal, terminate);
    process.kill(process.pid, signal);
  };
  process.once(signal, terminate);
}
