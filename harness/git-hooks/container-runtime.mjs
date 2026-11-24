import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

export const HOME_RUNTIME_PATHS = [
  ".colima/default/docker.sock",
  ".docker/run/docker.sock",
  ".docker/bin/docker",
  ".orbstack/run/docker.sock",
  ".orbstack/bin/docker",
  ".rd/docker.sock",
  ".rd/bin/docker",
];

export const detectUserContainerRuntime = ({
  dockerHost = process.env.DOCKER_HOST,
  homeDirectory = homedir(),
  exists = existsSync,
} = {}) => {
  if (typeof dockerHost === "string" && dockerHost.trim().length > 0) {
    return "present";
  }
  if (typeof homeDirectory !== "string" || homeDirectory.trim().length === 0) {
    return "present";
  }
  if (
    HOME_RUNTIME_PATHS.some((relative) => exists(join(homeDirectory, relative)))
  ) {
    return "present";
  }
  return "absent";
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.stdout.write(`${detectUserContainerRuntime()}\n`);
}
