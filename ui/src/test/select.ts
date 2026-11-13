import { fireEvent } from '@testing-library/react';

/** Lista aberta de uma lista de seleção do app (#479). */
function listbox(combobox: HTMLElement): HTMLElement | null {
  const id = combobox.getAttribute('aria-controls');
  return id ? document.getElementById(id) : null;
}

function open(combobox: HTMLElement): HTMLElement | null {
  if (combobox.getAttribute('aria-expanded') !== 'true') fireEvent.click(combobox);
  return listbox(combobox);
}

function close(combobox: HTMLElement) {
  if (combobox.getAttribute('aria-expanded') === 'true') fireEvent.keyDown(combobox, { key: 'Escape' });
}

/**
 * Escolhe uma opção pelo valor, como o usuário: abre a lista e clica na opção.
 * Com a lista desabilitada ou a opção desabilitada, nada muda (como no `<select>` nativo).
 */
export function chooseOption(combobox: HTMLElement, value: string | number) {
  if ((combobox as HTMLButtonElement).disabled || combobox.closest('fieldset:disabled')) return;
  const list = open(combobox);
  const option = list?.querySelector<HTMLElement>(`[role="option"][data-value="${String(value)}"]`);
  if (!option) throw new Error(`opção ${String(value)} não existe na lista ${combobox.id}`);
  fireEvent.click(option);
  close(combobox);
}

/** Opções da lista (abre, lê e fecha). */
function listOptions(combobox: HTMLElement): HTMLElement[] {
  const list = open(combobox);
  const options = list ? Array.from(list.querySelectorAll<HTMLElement>('[role="option"]')) : [];
  close(combobox);
  return options;
}

/** Textos das opções da lista, na ordem. */
export function optionLabels(combobox: HTMLElement): (string | null)[] {
  return listOptions(combobox).map(option => option.textContent);
}

/** Opção pelo texto (abre, lê e fecha); `undefined` se não existe. */
export function optionByLabel(combobox: HTMLElement, label: string): HTMLElement | undefined {
  return listOptions(combobox).find(option => option.textContent === label);
}
