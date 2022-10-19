export const deepEqual = <T>(a: T, b: T): boolean => {
  if (a === b) return true;
  if (a == null || b == null) return false;
  if (a.constructor !== b.constructor) return false;
  if (a instanceof Map && b instanceof Map) {
    if (a.size !== b.size) return false;
    for (const [key, value] of a) {
      if (!b.has(key)) return false;
      if (!deepEqual(value, b.get(key))) return false;
    }
    return true;
  }
  if (a instanceof Set && b instanceof Set) {
    if (a.size !== b.size) return false;
    for (const value of a) {
      if (!b.has(value)) return false;
    }
    return true;
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!deepEqual(a[i], b[i])) return false;
    }
    return true;
  }
  if (a instanceof Date && b instanceof Date) {
    return a.getTime() === b.getTime();
  }
  if (a instanceof RegExp && b instanceof RegExp) {
    return a.toString() === b.toString();
  }
  if (a.valueOf !== Object.prototype.valueOf) return a.valueOf() === b.valueOf();
  if (a.toString !== Object.prototype.toString) return a.toString() === b.toString();
  const keys = Object.keys(a);
  if (keys.length !== Object.keys(b).length) return false;
  return keys.every(k => deepEqual(a[k], b[k]));
};

export const deepClone = <T>(a: T): T => {
  if (a == null) return a;
  if (a instanceof Map) {
    const b = new Map();
    for (const [key, value] of a) {
      b.set(deepClone(key), deepClone(value));
    }
    return b;
  }
  if (a instanceof Set) {
    const b = new Set();
    for (const value of a) {
      b.add(deepClone(value));
    }
    return b;
  }
  if (Array.isArray(a)) {
    return a.map(deepClone);
  }
  if (a instanceof Date) {
    return new Date(a.getTime());
  }
  if (a instanceof RegExp) {
    return new RegExp(a);
  }
  if (a.valueOf !== Object.prototype.valueOf) return a.valueOf();
  if (a.toString !== Object.prototype.toString) return a.toString();
  const b = {};
  for (const key of Object.keys(a)) {
    (b as any)[key] = deepClone(a[key]);
  }
  return b;
};
