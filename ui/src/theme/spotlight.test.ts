import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installCardSpotlight } from './spotlight';

let cleanup: () => void;
let fine: { matches: boolean; addEventListener: ReturnType<typeof vi.fn>; removeEventListener: ReturnType<typeof vi.fn> };
let reduced: typeof fine;
function move(target: Element, x = 30, y = 50, type = 'mouse') {
  const event = new Event('pointermove', { bubbles: true });
  Object.assign(event, { clientX: x, clientY: y, pointerType: type });
  target.dispatchEvent(event);
}
beforeEach(() => {
  fine = { matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() };
  reduced = { ...fine, matches: false };
  vi.stubGlobal('matchMedia', (query: string) => query.includes('reduced-motion') ? reduced : fine);
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => setTimeout(() => callback(0), 16));
  vi.stubGlobal('cancelAnimationFrame', clearTimeout);
  vi.useFakeTimers();
  document.body.innerHTML = '<div class="card" id="first"><button>Action</button></div><div class="card" id="second"></div><aside class="app-sidebar"><div class="card" id="social"></div></aside>';
  cleanup = installCardSpotlight();
});
afterEach(() => { cleanup(); document.body.innerHTML = ''; vi.useRealTimers(); vi.unstubAllGlobals(); });

it('segue a última posição uma vez por quadro, sem impedir cliques', () => {
  const card = document.querySelector<HTMLElement>('#first')!;
  vi.spyOn(card, 'getBoundingClientRect').mockReturnValue({ left: 10, top: 20 } as DOMRect);
  const click = vi.fn();
  card.querySelector('button')!.onclick = click;
  move(card.querySelector('button')!);
  move(card, 60, 80);
  expect(vi.getTimerCount()).toBe(1);
  vi.advanceTimersByTime(16);
  expect(card.style.getPropertyValue('--el-spotlight-x')).toBe('50px');
  expect(card.style.getPropertyValue('--el-spotlight-y')).toBe('60px');
  card.querySelector('button')!.click();
  expect(click).toHaveBeenCalledOnce();
});
it('limpa o card anterior, as barras e o efeito após rolar', () => {
  const first = document.querySelector('#first')!;
  const second = document.querySelector('#second')!;
  move(first); vi.advanceTimersByTime(16);
  move(second); vi.advanceTimersByTime(16);
  expect(first.hasAttribute('data-spotlight')).toBe(false);
  expect(second.hasAttribute('data-spotlight')).toBe(true);
  document.dispatchEvent(new Event('scroll'));
  expect(second.hasAttribute('data-spotlight')).toBe(false);
  move(document.querySelector('#social')!); vi.advanceTimersByTime(16);
  expect(document.querySelector('[data-spotlight]')).toBeNull();
});
it('respeita movimento reduzido, toque e ausência de hover', () => {
  const card = document.querySelector('#first')!;
  reduced.matches = true; move(card);
  reduced.matches = false; move(card, 0, 0, 'touch');
  fine.matches = false; move(card);
  vi.advanceTimersByTime(16);
  expect(card.hasAttribute('data-spotlight')).toBe(false);
});
it('cancela atualizações e listeners ao desmontar, inclusive no StrictMode', () => {
  const card = document.querySelector('#first')!;
  move(card); cleanup();
  vi.advanceTimersByTime(16);
  move(card); vi.advanceTimersByTime(16);
  expect(card.hasAttribute('data-spotlight')).toBe(false);
  cleanup = installCardSpotlight();
  move(card); vi.advanceTimersByTime(16);
  expect(card.hasAttribute('data-spotlight')).toBe(true);
  cleanup();
  expect(card.hasAttribute('data-spotlight')).toBe(false);
});

it('mantém o último ponto ao sair enquanto a opacidade faz fade-out', () => {
  const card = document.querySelector<HTMLElement>('#first')!;
  move(card, 120, 80); vi.advanceTimersByTime(16);
  const out = new Event('pointerout', { bubbles: true });
  Object.assign(out, { relatedTarget: document.body });
  card.dispatchEvent(out);
  expect(card.hasAttribute('data-spotlight')).toBe(false);
  vi.advanceTimersByTime(90); // Meio da transição CSS de 180 ms.
  expect(card.style.getPropertyValue('--el-spotlight-x')).toBe('120px');
  expect(card.style.getPropertyValue('--el-spotlight-y')).toBe('80px');
});

it('preserva o ponto do card anterior e atualiza a posição na reentrada', () => {
  const first = document.querySelector<HTMLElement>('#first')!;
  const second = document.querySelector<HTMLElement>('#second')!;
  move(first, 80, 60); vi.advanceTimersByTime(16);
  move(second, 200, 100); vi.advanceTimersByTime(16);
  expect(first.style.getPropertyValue('--el-spotlight-x')).toBe('80px');
  expect(first.hasAttribute('data-spotlight')).toBe(false);
  expect(second.hasAttribute('data-spotlight')).toBe(true);
  move(first, 40, 20); vi.advanceTimersByTime(16);
  expect(first.style.getPropertyValue('--el-spotlight-x')).toBe('40px');
  expect(first.hasAttribute('data-spotlight')).toBe(true);
  expect(second.style.getPropertyValue('--el-spotlight-x')).toBe('200px');
  cleanup();
  for (const card of [first, second]) {
    expect(card.style.getPropertyValue('--el-spotlight-x')).toBe('');
    expect(card.style.getPropertyValue('--el-spotlight-y')).toBe('');
  }
});
