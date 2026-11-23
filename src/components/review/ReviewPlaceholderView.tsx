import { ListChecks } from "lucide-react";

export function ReviewPlaceholderView() {
  return (
    <section className="content-surface flex flex-1 flex-col items-center justify-center rounded-[24px] p-8 text-center">
      <div className="liquid-glass mb-5 flex h-16 w-16 items-center justify-center rounded-[20px]">
        <ListChecks className="relative z-10 h-7 w-7 text-sky-400" strokeWidth={1.85} />
      </div>
      <h3 className="text-[19px] font-semibold tracking-tight text-primary">
        File de révision post-traitement
      </h3>
      <p className="mt-2 max-w-md text-[13px] leading-relaxed text-secondary">
        Les pistes au titre cryptique, sans pochette ou en conflit de version
        (Live, Remix, Sped Up…) apparaîtront ici sans jamais bloquer vos lots.
        L&apos;édition manuelle et le lecteur comparateur A/B arriveront au
        Sprint 2.
      </p>
    </section>
  );
}
