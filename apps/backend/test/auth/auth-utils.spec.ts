import {
  createSessionToken,
  extractBearerToken,
  hashPassword,
  hashSessionToken,
  verifyPassword,
} from '../../src/modules/auth/auth.utils';

describe('auth.utils', () => {
  it('hashes and verifies passwords', async () => {
    const password = 'Password123!';
    const hash = await hashPassword(password);

    expect(hash).not.toBe(password);
    await expect(verifyPassword(password, hash)).resolves.toBe(true);
    await expect(verifyPassword('WrongPassword123!', hash)).resolves.toBe(false);
  });

  it('rejects malformed stored hashes', async () => {
    await expect(verifyPassword('Password123!', 'invalid')).resolves.toBe(false);
  });

  it('creates stable token hashes and parses bearer tokens', () => {
    const token = createSessionToken();

    expect(token).toBeTruthy();
    expect(hashSessionToken(token)).toHaveLength(64);
    expect(hashSessionToken(token)).toBe(hashSessionToken(token));
    expect(extractBearerToken(`Bearer ${token}`)).toBe(token);
    expect(extractBearerToken(`Basic ${token}`)).toBeUndefined();
  });
});
