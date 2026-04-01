import { BadRequestException } from '@nestjs/common';
import { z } from 'zod';

const emailSchema = z.string().trim().email().transform((value) => value.toLowerCase());
const passwordSchema = z
  .string()
  .min(8, 'Password must be at least 8 characters')
  .max(72, 'Password must be at most 72 characters');

export const registerSchema = z.object({
  email: emailSchema,
  password: passwordSchema,
  displayName: z.string().trim().min(1).max(80).optional(),
});

export const loginSchema = z.object({
  email: emailSchema,
  password: passwordSchema,
});

export type RegisterInput = z.infer<typeof registerSchema>;
export type LoginInput = z.infer<typeof loginSchema>;

export const parseAuthInput = <T>(schema: z.ZodType<T>, payload: unknown): T => {
  const result = schema.safeParse(payload);

  if (result.success) {
    return result.data;
  }

  const flattened = result.error.flatten();
  throw new BadRequestException({
    message: 'Invalid auth payload',
    errors: {
      formErrors: flattened.formErrors,
      fieldErrors: flattened.fieldErrors,
    },
  });
};
