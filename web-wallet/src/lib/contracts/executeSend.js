import { getLastTransactionHash } from "$lib/transactions";
import { walletStore } from "$lib/stores";

/** @type {(to: string, amount: number, gasPrice:number, gasLimit:number) => Promise<string>} */
const executeSend = (to, amount, gasPrice, gasLimit) =>
  walletStore
    .transfer(to, amount, gasPrice, gasLimit)
    .then(getLastTransactionHash);

export default executeSend;
