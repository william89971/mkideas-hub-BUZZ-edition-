import { mkdirSync } from "node:fs";
import { expect, test } from "@playwright/test";

import type { RelayEvent } from "../../src/shared/api/types";
import { waitForAnimations } from "../helpers/animations";
import { installMockBridge } from "../helpers/bridge";

const OUTPUT = "test-results/mkideas-v0";
const HOST = "localhost:3000";
const GUEST_ID = "11111111-1111-4111-8111-111111111111";
const INTERVIEW_ID = "22222222-2222-4222-8222-222222222222";
const CONTENT_ID = "33333333-3333-4333-8333-333333333333";

async function waitForMockLiveSubscription(
  page: import("@playwright/test").Page,
  channelName: string,
) {
  await expect
    .poll(() =>
      page.evaluate(
        (name) =>
          window.__BUZZ_E2E_HAS_MOCK_LIVE_SUBSCRIPTION__?.({
            channelName: name,
          }) ?? false,
        channelName,
      ),
    )
    .toBe(true);
}

function event(
  idDigit: string,
  kind: number,
  content: Record<string, unknown>,
  tags: string[][],
  createdAt: number,
  pubkey = "a".repeat(64),
): RelayEvent {
  return {
    id: idDigit.repeat(64),
    pubkey,
    created_at: createdAt,
    kind,
    tags: [["h", HOST], ...tags],
    content: JSON.stringify(content),
    sig: "f".repeat(128),
  };
}

function fixtures(): RelayEvent[] {
  const now = 1_788_000_000;
  return [
    event(
      "1",
      30803,
      {
        schema_version: 1,
        record_type: "person",
        entity_id: GUEST_ID,
        version: 1,
        status: "research_ready",
        name: "Maya Chen",
        organization: "Signal & Story",
        why_now: "A timely conversation about building trust in AI media.",
        do_not_contact: false,
      },
      [
        ["d", GUEST_ID],
        ["version", "1"],
        ["status", "research_ready"],
      ],
      now - 300,
    ),
    event(
      "2",
      48201,
      {
        schema_version: 1,
        proposal_id: "44444444-4444-4444-8444-444444444444",
        proposal_version: 1,
        target_id: GUEST_ID,
        target_kind: 30803,
        agent: "Guest Researcher",
        proposal_type: "guest_research",
        summary:
          "Maya connects community journalism, transparent AI sourcing, and practical audience trust—a strong fit for the next MK Ideas conversation.",
        provenance: [
          "Signal & Story public profile",
          "Recent Local Media Lab talk",
          "MK Ideas guest criteria v1",
        ],
        status: "proposed",
      },
      [
        ["status", "proposed"],
        ["target", "30803", GUEST_ID],
      ],
      now - 240,
      "b".repeat(64),
    ),
    event(
      "3",
      30804,
      {
        schema_version: 1,
        record_type: "interview",
        entity_id: INTERVIEW_ID,
        version: 2,
        status: "content_processing",
        title: "Interview with Maya Chen",
        guest_id: GUEST_ID,
        transcript_name: "maya-chen.vtt",
        transcript_text:
          "00:02:14.000 --> 00:02:48.000 Trust begins when sources remain visible.",
      },
      [
        ["d", INTERVIEW_ID],
        ["version", "2"],
        ["status", "content_processing"],
        ["guest", GUEST_ID],
      ],
      now - 180,
    ),
    event(
      "4",
      30805,
      {
        schema_version: 1,
        record_type: "content",
        entity_id: CONTENT_ID,
        version: 1,
        status: "internal_review",
        title: "Maya Chen — trust is visible",
        interview_id: INTERVIEW_ID,
      },
      [
        ["d", CONTENT_ID],
        ["version", "1"],
        ["status", "internal_review"],
        ["interview", INTERVIEW_ID],
      ],
      now - 120,
    ),
    event(
      "5",
      48201,
      {
        schema_version: 1,
        proposal_id: "55555555-5555-4555-8555-555555555555",
        proposal_version: 1,
        target_id: CONTENT_ID,
        target_kind: 30805,
        agent: "Content / Clip Copilot",
        proposal_type: "timestamped_clips",
        summary:
          "Two concise moments are ready for a partner’s editorial judgment.",
        provenance: ["maya-chen.vtt sha256:demo", "Content record v1"],
        clips: [
          {
            start: "00:02:14.000",
            end: "00:02:48.000",
            title: "Trust begins with visible sources",
            caption:
              "If people can trace the source, AI can strengthen—not weaken—editorial trust.",
          },
          {
            start: "00:18:03.000",
            end: "00:18:31.000",
            title: "Make the process legible",
            caption:
              "The strongest media systems explain how a story came together.",
          },
        ],
        status: "proposed",
      },
      [
        ["status", "proposed"],
        ["target", "30805", CONTENT_ID],
      ],
      now - 60,
      "c".repeat(64),
    ),
  ];
}

test.beforeEach(async ({ page }) => {
  const seeded = fixtures();
  await page.addInitScript((events) => {
    window.__BUZZ_E2E_MKIDEAS_EVENTS__ = events;
  }, seeded);
  await installMockBridge(page);
});

test("V0 guest-to-content workspace is coherent across the four active areas", async ({
  page,
}) => {
  mkdirSync(OUTPUT, { recursive: true });
  await page.setViewportSize({ width: 1440, height: 900 });

  await page.goto("/#/people");
  await expect(page.getByTestId("mkideas-people")).toBeVisible();
  await expect(page.getByText("Maya Chen", { exact: true })).toBeVisible();
  await expect(page.getByText(/Attached research/i)).toBeVisible();
  await expect(page.getByText(/3 provenance sources/i)).toBeVisible();
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/01-people-research.png` });

  await page.goto("/#/");
  await expect(page.getByTestId("mkideas-today")).toBeVisible();
  await expect(page.getByText("2", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("AI cannot clear this list")).toBeVisible();
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/02-today-human-gates.png` });

  await page.goto("/#/studio");
  await expect(page.getByTestId("mkideas-studio")).toBeVisible();
  await expect(
    page.getByText("Interview with Maya Chen", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Trust begins with visible sources"),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Approve" })).toBeVisible();
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/03-studio-review.png` });

  await expect(page.getByTestId("open-work-view")).toBeVisible();
  await expect(page.getByTestId("open-people-view")).toBeVisible();
  await expect(page.getByTestId("open-studio-view")).toBeVisible();
  await expect(page.getByTestId("open-team-view")).toBeVisible();

  await page.getByTestId("channel-general").click();
  await waitForMockLiveSubscription(page, "general");
  const recordLink = `buzz://mkideas?kind=30805&id=${CONTENT_ID}&proposal=55555555-5555-4555-8555-555555555555`;
  await page.evaluate((link) => {
    window.__BUZZ_E2E_EMIT_MOCK_MESSAGE__?.({
      channelName: "general",
      content: `Review discussion for the Maya Chen clip proposal\n${link}`,
    });
  }, recordLink);
  await expect(
    page.getByText("Review discussion for the Maya Chen clip proposal"),
  ).toBeVisible();
  const teamRecordLink = page.locator('[data-buzz-link-kind="mkideas"]');
  await expect(teamRecordLink).toBeVisible();
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/04-team-linked-discussion.png` });
  await teamRecordLink.click();
  await expect(page.getByTestId("mkideas-studio")).toBeVisible();
});
