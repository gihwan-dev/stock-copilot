if (process.env.MUTATION_STATUS !== "passed") {
  process.stderr.write("변경분 변이가 완주하지 않았다\n");
  process.exitCode = 1;
}
