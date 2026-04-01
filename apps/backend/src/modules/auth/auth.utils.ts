import { createHash, randomBytes, scrypt as nodeScrypt, timingSafeEqual } from 'node:crypto';
import { promisify } from 'node:util';

const scrypt = promisify(nodeScrypt);
const PASSWORD_KEYLEN = 64;
const PASSWORD_PREFIX = 'scrypt';
const SESSION_TOKEN_BYTES = 32;

export const createSessionToken = () => randomBytes(SESSION_TOKEN_BYTES).toString('base64url');

export const hashSessionToken = (token: string) =>
  createHash('sha256').update(token).digest('hex');

export const hashPassword = async (password: string) => {
  const salt = randomBytes(16).toString('base64url');
  const derivedKey = (await scrypt(password, salt, PASSWORD_KEYLEN)) as Buffer;
  return `${PASSWORD_PREFIX}$${salt}$${derivedKey.toString('base64url')}`;
};

export const verifyPassword = async (password: string, storedHash: string) => {
  const [scheme, salt, expectedHash] = storedHash.split('$');

  if (scheme !== PASSWORD_PREFIX || !salt || !expectedHash) {
    return false;
  }

  const derivedKey = (await scrypt(password, salt, PASSWORD_KEYLEN)) as Buffer;
  const expectedKey = Buffer.from(expectedHash, 'base64url');

  if (derivedKey.length !== expectedKey.length) {
    return false;
  }

  return timingSafeEqual(derivedKey, expectedKey);
};

export const extractBearerToken = (authorization?: string) => {
  if (!authorization) {
    return undefined;
  }

  const [type, token] = authorization.split(' ');
  return type === 'Bearer' && token ? token : undefined;
};
