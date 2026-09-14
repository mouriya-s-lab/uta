// Expected TS2345: a class-valued output cannot enter a JSON read projection.
import { z } from 'zod';
import { readProjectionResponseSchema } from '../../wire/projection.ts';
readProjectionResponseSchema(z.strictObject({observed:z.date()}));
