import { useEffect } from 'react';
import { installCardSpotlight } from './spotlight';

export function CardSpotlight() {
  useEffect(installCardSpotlight, []);
  return null;
}
