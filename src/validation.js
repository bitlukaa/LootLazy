const path = require('path');
function validateExecutable(value) {
  if (typeof value !== 'string' || value.length > 240) throw new Error('Invalid executable path');
  const normalized = value.trim().replace(/\\/g, '/');
  if (!normalized || normalized.startsWith('/') || /[:<>"|?*\x00-\x1f]/.test(normalized) || !/\.exe$/i.test(normalized) || normalized.split('/').some(p => !p || p === '.' || p === '..' || /[. ]$/.test(p) || /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(p))) throw new Error('Use a relative Windows .exe path without traversal');
  return normalized;
}
function validateAppId(value) {
  if (typeof value !== 'string' || !/^\d{5,22}$/.test(value)) throw new Error('Invalid application ID');
  return value;
}
function targetPath(root, id, executable) {
  const result = path.resolve(root, validateAppId(id), validateExecutable(executable));
  if (!result.startsWith(path.resolve(root) + path.sep)) throw new Error('Path outside games directory');
  return result;
}
module.exports = { validateExecutable, validateAppId, targetPath };
