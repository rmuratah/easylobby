import type { MouseEvent } from 'react';
import { sound } from './sound';

/**
 * Toque leve ao passar o mouse num dos botões principais (#501): Iniciar
 * busca, Criar lobby, Aceitar, Recusar, Banir e Confirmar. Botão desabilitado
 * não toca. O clique toca no próprio `onClick` de cada botão, para valer
 * também pelo teclado.
 */
export function hoverSound(event: MouseEvent<HTMLButtonElement>) {
  if (!event.currentTarget.disabled) sound.play('hover');
}
