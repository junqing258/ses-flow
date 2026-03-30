import { Module, Global } from '@nestjs/common';
import { ConfigModule, ConfigService } from '@nestjs/config';
import { createClient, SupabaseClient } from '@supabase/supabase-js';
import { SupabaseGuard } from './supabase.guard';

@Global()
@Module({
  imports: [ConfigModule],
  providers: [
    {
      provide: 'SUPABASE_CLIENT',
      useFactory: (configService: ConfigService) => {
        return createClient(
          configService.get<string>('SUPABASE_URL')!,
          configService.get<string>('SUPABASE_KEY')!,
        );
      },
      inject: [ConfigService],
    },
    SupabaseGuard,
  ],
  exports: ['SUPABASE_CLIENT', SupabaseGuard],
})
export class AuthModule { }
