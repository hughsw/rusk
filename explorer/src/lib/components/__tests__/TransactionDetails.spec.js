import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import { TransactionDetails } from "../";

global.ResizeObserver = vi.fn().mockImplementation(() => ({
  disconnect: vi.fn(),
  observe: vi.fn(),
  unobserve: vi.fn(),
}));

const baseProps = {
  data: {
    blockhash:
      "42bb6382d5e7e4fe794c548a0bcf9634b8322aebb31e4f7eb5a16ea86a5ae933",
    blockheight: 488582,
    blocktimestamp: "2024-04-16 08:55:36 +0000 UTC",
    blockts: 1,
    contract: "Transfer",
    feepaid: 292491,
    gaslimit: 500000000,
    gasprice: 1,
    gasspent: 292491,
    method: "transfer",
    success: true,
    txerror: "",
    txid: "4e49cd5a2f8c6eb8b1f09700f06e5bfa3fbf25591c322eae59428c25d1c04a07",
    txtype: "1",
  },
};

describe("Transaction Details", () => {
  afterEach(cleanup);

  it("renders the Transaction Details component", () => {
    const { container } = render(TransactionDetails, baseProps);

    expect(container.firstChild).toMatchSnapshot();
  });
});