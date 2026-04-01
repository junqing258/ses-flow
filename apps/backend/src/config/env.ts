import path from 'node:path';
import { z } from 'zod';

const envSchema = z.object({
  NODE_ENV: z.enum(['development', 'test', 'production']).optional(),
  PORT: z.coerce.number().int().positive().default(3000),
  DATABASE_URL: z.string().min(1, 'DATABASE_URL is required'),
  DIRECT_URL: z.string().min(1, 'DIRECT_URL is required'),
});

export const validateEnv = (rawEnv: Record<string, unknown>) => envSchema.parse(rawEnv);

export const resolveEnvFilePaths = () => [
  path.resolve(process.cwd(), 'apps/backend/.env'),
  path.resolve(process.cwd(), '.env'),
];
