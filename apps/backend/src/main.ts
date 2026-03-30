import 'reflect-metadata';
import { existsSync } from 'node:fs';
import path from 'node:path';
import fastifyStatic from '@fastify/static';
import { NestFactory } from '@nestjs/core';
import { FastifyAdapter, NestFastifyApplication } from '@nestjs/platform-fastify';
import { AppModule } from './modules/app.module';

const resolveWebDist = () => {
  const candidates = [
    path.resolve(process.cwd(), 'web/dist'),
    path.resolve(process.cwd(), '../web/dist'),
  ];

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }

  return null;
};

async function bootstrap() {
  const app = await NestFactory.create<NestFastifyApplication>(
    AppModule,
    new FastifyAdapter(),
  );

  app.setGlobalPrefix('api');
  app.enableCors();

  const webDist = resolveWebDist();
  if (webDist) {
    const fastify = app.getHttpAdapter().getInstance();
    await fastify.register(fastifyStatic, {
      root: webDist,
      prefix: '/api/advisor/ui/',
    });
  } else {
    console.warn('Advisor UI dist not found. Run the Vite build to enable /api/advisor/ui.');
  }

  await app.listen({ port: 3000, host: '0.0.0.0' });
}

bootstrap();
