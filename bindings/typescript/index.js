const fs = require('node:fs')
const path = require('node:path')

function platformKey() {
  if (process.platform === 'linux') {
    const libc = process.report && process.report.getReport && process.report.getReport().header
      ? process.report.getReport().header.glibcVersionRuntime
        ? 'gnu'
        : 'musl'
      : 'gnu'
    return `linux-${process.arch}-${libc}`
  }
  if (process.platform === 'darwin') {
    return `darwin-${process.arch}`
  }
  if (process.platform === 'win32') {
    return `win32-${process.arch}-msvc`
  }
  return `${process.platform}-${process.arch}`
}

function localCandidates() {
  const dir = __dirname
  const files = fs.readdirSync(dir)
  return files
    .filter((name) => name.startsWith('ipcprims.') && name.endsWith('.node'))
    .map((name) => path.join(dir, name))
}

function packageNameForKey(key) {
  switch (key) {
    case 'linux-x64-gnu':
      return '@3leaps/ipcprims-linux-x64-gnu'
    case 'linux-x64-musl':
      return '@3leaps/ipcprims-linux-x64-musl'
    case 'linux-arm64-gnu':
      return '@3leaps/ipcprims-linux-arm64-gnu'
    case 'darwin-arm64':
      return '@3leaps/ipcprims-darwin-arm64'
    case 'win32-x64-msvc':
      return '@3leaps/ipcprims-win32-x64-msvc'
    default:
      return null
  }
}

function loadBinding() {
  for (const candidate of localCandidates()) {
    try {
      return require(candidate)
    } catch {
    }
  }

  const key = platformKey()
  const pkg = packageNameForKey(key)
  if (pkg) {
    try {
      return require(pkg)
    } catch {
    }
  }

  throw new Error(`Unable to load @3leaps/ipcprims native binding for ${key}`)
}

const native = loadBinding()

module.exports = {
  Listener: native.Listener,
  Peer: native.Peer,
  SchemaRegistry: native.SchemaRegistry,
  CONTROL: native.control(),
  COMMAND: native.command(),
  DATA: native.data(),
  TELEMETRY: native.telemetry(),
  ERROR: native.error()
}
