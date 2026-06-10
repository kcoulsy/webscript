import type * as PrismNamespace from 'prismjs';

import {registerWebLanguage} from '../prism/web';
import prismIncludeLanguagesOriginal from '@theme-original/prism-include-languages';

export default function prismIncludeLanguages(
  PrismObject: typeof PrismNamespace,
): void {
  prismIncludeLanguagesOriginal(PrismObject);
  registerWebLanguage(PrismObject);
}
