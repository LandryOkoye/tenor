import { useState } from "react";
import { Link } from "react-router";
import { Dialog } from "@/components/ui/Dialog";
import { Button } from "@/components/ui/Button";
import { readStore, writeStore } from "@/lib/storage";

const KEY = "tenor.age-ok";

/** 18+ confirmation and country notice (brief §12, §14). */
export function AgeGate() {
  const [ok, setOk] = useState(() => readStore<boolean>(KEY, false));
  const [declined, setDeclined] = useState(false);
  return (
    <Dialog open={!ok} onClose={() => {}} title="Before you continue" dismissable={false}>
      {declined ? (
        <p className="text-sm text-muted">Tenor is only available to adults aged 18 and over. Come back when you are.</p>
      ) : (
        <>
          <div className="space-y-3 text-sm text-muted">
            <p>Prediction markets involve real money and real risk. You can lose what you put in, and positions can't be sold before a market resolves.</p>
            <p>
              Tenor is not available in some countries. By continuing you confirm you are <b className="text-ink">18 or older</b> and that using prediction markets is legal where you live.
            </p>
          </div>
          <div className="mt-6 flex flex-col gap-2 sm:flex-row-reverse">
            <Button className="flex-1" onClick={() => { writeStore(KEY, true); setOk(true); }}>
              I'm 18+, continue
            </Button>
            <Button variant="secondary" className="flex-1" onClick={() => setDeclined(true)}>
              I'm under 18
            </Button>
          </div>
          <p className="mt-4 text-xs text-muted">
            Read the <Link to="/legal" className="underline">risk disclosure and terms</Link>.
          </p>
        </>
      )}
    </Dialog>
  );
}
