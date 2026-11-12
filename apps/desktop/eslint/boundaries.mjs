export const ELEMENTS = [
  { type: "app", pattern: "src/app" },
  { type: "feature", pattern: "src/features/*", capture: ["featureName"] },
  { type: "ds", pattern: "src/shared/ds" },
  { type: "ipc", pattern: "src/shared/ipc" },
  { type: "platform", pattern: "src/shared/platform" },
  { type: "lib", pattern: "src/shared/lib" },
].map((element) => ({ ...element, partialMatch: false }));

export const ENTRY_FILE = "index.ts";

export const ELEMENT_DEPENDENCIES = {
  app: ["feature", "ds", "ipc", "platform", "lib"],
  feature: ["ds", "ipc", "platform", "lib"],
  ds: ["lib"],
  ipc: ["lib"],
  platform: ["ipc", "lib"],
  lib: [],
};

export const PACKAGE_LOCATIONS = {
  "@tauri-apps/api": ["src/shared/ipc/**"],
  zod: ["src/shared/ipc/**", "src/shared/lib/brand.ts"],
};

export const BOUNDARY_TABLES = {
  ELEMENTS,
  ENTRY_FILE,
  ELEMENT_DEPENDENCIES,
  PACKAGE_LOCATIONS,
};

const elementPolicies = (dependencies, entryFile) =>
  Object.entries(dependencies)
    .filter(([, targets]) => targets.length > 0)
    .map(([type, targets]) => ({
      from: { element: { type } },
      allow: {
        to: { element: { types: targets, fileInternalPath: entryFile } },
      },
    }));

export const BOUNDARY_MESSAGES = {
  element:
    "{{ from.element.types.[0] }}에서 {{ to.element.types.[0] }}의 {{ to.element.fileInternalPath }} 가져오기 금지: architecture 5장 폴더 의존 표의 방향으로, 다른 요소는 index.ts로만 가져온다.",
  package:
    "{{ to.module.source }} 가져오기 금지: 이 패키지는 정해진 위치의 파일에서만 가져온다.",
  core: "Node 내장 모듈 {{ to.module.source }} 가져오기 금지: UI는 IPC로만 바깥과 통신한다.",
  unknown:
    "src의 요소(app, features/*, shared/*) 밖 파일 {{ dependency.source }} 가져오기 금지.",
};

const packagePolicies = (locations) =>
  Object.entries(locations).flatMap(([source, paths]) => [
    {
      disallow: { to: { module: { origin: "external", source } } },
      message: BOUNDARY_MESSAGES.package,
    },
    {
      from: { file: { path: paths } },
      allow: { to: { module: { origin: "external", source } } },
    },
  ]);

export const boundaryPolicies = ({
  ENTRY_FILE: entryFile,
  ELEMENT_DEPENDENCIES: dependencies,
  PACKAGE_LOCATIONS: locations,
}) => [
  ...elementPolicies(dependencies, entryFile),
  { allow: { to: { module: { origin: "external" } } } },
  ...packagePolicies(locations),
  {
    disallow: { to: { module: { origin: "core" } } },
    message: BOUNDARY_MESSAGES.core,
  },
  {
    disallow: {
      to: { module: { origin: "local" }, element: { isUnknown: true } },
    },
    message: BOUNDARY_MESSAGES.unknown,
  },
];

export const boundarySettings = ({ ELEMENTS: elements }, rootPath) => ({
  "boundaries/root-path": rootPath,
  "boundaries/legacy-templates": false,
  "boundaries/elements": elements,
  "import/resolver": { node: { extensions: [".ts", ".tsx"] } },
});

export const boundaryRules = (tables) => ({
  "boundaries/no-unknown-files": "error",
  "boundaries/no-unknown-dependencies": "error",
  "boundaries/dependencies": [
    "error",
    {
      default: "disallow",
      message: BOUNDARY_MESSAGES.element,
      checkAllOrigins: true,
      checkUnknownLocals: true,
      policies: boundaryPolicies(tables),
    },
  ],
});
