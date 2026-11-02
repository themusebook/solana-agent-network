import fs from 'fs'; import crypto from 'crypto';
import { createUmi } from '@metaplex-foundation/umi-bundle-defaults';
import { generateSigner, keypairIdentity, sol, publicKey, createSignerFromKeypair, transactionBuilder } from '@metaplex-foundation/umi';
import { mplCore, createCollection, create } from '@metaplex-foundation/mpl-core';
import { mplAgentIdentity, registerIdentityV1 } from '@metaplex-foundation/mpl-agent-registry';
import { transferSol, addMemo, mplToolbox } from '@metaplex-foundation/mpl-toolbox';
import bs58 from 'bs58';
const umi = createUmi('https://api.devnet.solana.com').use(mplCore()).use(mplAgentIdentity()).use(mplToolbox());
const payerKp = umi.eddsa.createKeypairFromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/workspace/.secrets/payer.json'))));
const payer = createSignerFromKeypair(umi, payerKp);
const w = generateSigner(umi); umi.use(keypairIdentity(w));
const sig = r => bs58.encode(r.signature);
const bal = async () => Number((await umi.rpc.getBalance(w.publicKey)).basisPoints);
const out = {wallet: w.publicKey.toString(), steps: []};
const step = async (name, b, extra={}) => { const b0 = await bal(); const r = await b.sendAndConfirm(umi); const b1 = await bal(); out.steps.push({name, sig: sig(r), lamports: b0-b1, ...extra}); };
// fund
umi.use(keypairIdentity(payer));
const f = await transferSol(umi,{source:payer,destination:w.publicKey,amount:sol(0.03)}).sendAndConfirm(umi); out.fund=sig(f);
umi.use(keypairIdentity(w));
const uri = 'https://registry.musebook.trade/SKILL.md';
const col = generateSigner(umi), asset = generateSigner(umi);
await step('baseline.createCollection', createCollection(umi,{collection:col,name:'SAN bench',uri}));
await step('baseline.createAsset', create(umi,{asset,collection:{publicKey:col.publicKey},name:'agent-1',uri}));
await step('baseline.registerIdentityV1', registerIdentityV1(umi,{asset:asset.publicKey,collection:col.publicKey,agentRegistrationUri:uri}));
// ours: second agent in the SAME shared collection, plus content-hash memo anchor
const asset2 = generateSigner(umi);
await step('shared.createAsset(no new collection)', create(umi,{asset:asset2,collection:{publicKey:col.publicKey},name:'agent-2',uri}));
const json = JSON.stringify({type:'https://eips.ethereum.org/EIPS/eip-8004#registration-v1',name:'agent-2',asset:asset2.publicKey.toString()});
const h = crypto.createHash('sha256').update(json).digest('hex');
await step('ours.memoAnchor(sha256 of registration JSON)', addMemo(umi,{memo:`san1:reg:${asset2.publicKey}:${h}`}), {sha256:h});
for (const [k,a] of [['collection',col],['asset',asset],['asset2',asset2]]) { const acc = await umi.rpc.getAccount(a.publicKey); out[k]={addr:a.publicKey.toString(),space:acc.data.length,lamports:Number(acc.lamports.basisPoints)}; }
// sweep
const left = await bal(); const sw = await transferSol(umi,{source:w,destination:payer.publicKey,amount:{basisPoints:BigInt(left-5000),identifier:'SOL',decimals:9}}).sendAndConfirm(umi); out.sweep={sig:sig(sw),lamports:left-5000};
console.log(JSON.stringify(out,null,1));
