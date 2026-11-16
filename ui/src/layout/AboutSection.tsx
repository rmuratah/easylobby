/** Aviso de não endosso que a política da Riot pede para o jogador ver (#217). */
export const RIOT_LEGAL_NOTICE =
  "easylobby isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games, and all associated properties are trademarks or registered trademarks of Riot Games, Inc.";

/** Seção "Sobre" do painel de Configurações, com o aviso legal da Riot (#217). */
export function AboutSection() {
  return (
    <section aria-labelledby="about-heading" className="flex flex-col gap-1 border-t border-line pt-3">
      <h2 id="about-heading" className="mb-1 text-section font-semibold">Sobre</h2>
      <p className="text-caption text-muted">easylobby é um app gratuito de fãs, sem vínculo com a Riot Games.</p>
      <p lang="en" className="text-caption text-muted">{RIOT_LEGAL_NOTICE}</p>
    </section>
  );
}
