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
  idSeed: string,
  kind: number,
  content: Record<string, unknown>,
  tags: string[][],
  createdAt: number,
  pubkey = "a".repeat(64),
): RelayEvent {
  return {
    id: idSeed.padStart(64, "0").slice(-64),
    pubkey,
    created_at: createdAt,
    kind,
    tags: [["h", HOST], ...tags],
    content: JSON.stringify(content),
    sig: "f".repeat(128),
  };
}

function fixtures(): RelayEvent[] {
  const now = Math.floor(Date.now() / 1000);
  const guestEventId = "1".padStart(64, "0");
  const interviewEventId = "3".padStart(64, "0");
  const contentEventId = "4".padStart(64, "0");
  const clipProposalId = "55555555-5555-4555-8555-555555555555";
  const clipProposalEventId = "5".padStart(64, "0");
  return [
    event(
      "1",
      30803,
      {
        schema_version: 2,
        record_type: "person",
        entity_id: GUEST_ID,
        version: 1,
        status: "ready-to-contact",
        name: "Maya Chen",
        title: "Founder and editor",
        organization: "Signal & Story",
        email: "maya@example.test",
        location: "Oakland, California",
        owner: "William",
        topics: ["editorial trust", "community media", "responsible AI"],
        why_now: "A timely conversation about building trust in AI media.",
        do_not_contact: false,
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: "maya-person-v1" },
      },
      [
        ["d", GUEST_ID],
        ["version", "1"],
        ["status", "ready-to-contact"],
      ],
      now - 300,
    ),
    event(
      "2",
      48201,
      {
        schema_version: 2,
        proposal_id: "44444444-4444-4444-8444-444444444444",
        proposal_version: 1,
        target_id: GUEST_ID,
        target_kind: 30803,
        target_event_id: guestEventId,
        target_version: 1,
        agent: "Guest Researcher",
        persona_id: "guest-researcher",
        run_id: "10000000-0000-4000-8000-000000000001",
        template_version: "guest-research.v2",
        provider: "fixture",
        model: "deterministic-local-v1",
        input_event_ids: [guestEventId],
        input_hash: "a".repeat(64),
        proposal_type: "guest_research",
        summary:
          "Maya connects community journalism, transparent AI sourcing, and practical audience trust—a strong fit for the next MK Ideas conversation.",
        provenance: [
          "Signal & Story public profile",
          "Recent Local Media Lab talk",
          "MK Ideas guest criteria v1",
        ],
        status: "proposed",
        review_state: "pending",
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
        schema_version: 2,
        record_type: "interview",
        entity_id: INTERVIEW_ID,
        version: 2,
        status: "reviewing",
        title: "Interview with Maya Chen",
        guest_id: GUEST_ID,
        scheduled_at: new Date((now + 2 * 86_400) * 1000).toISOString(),
        brief:
          "Explore how transparent sourcing can make AI-assisted media more trustworthy without automating editorial judgment.",
        questions: [
          "What changes when a newsroom keeps its sources visible?",
          "Where should human judgment remain explicit?",
        ],
        transcript_descriptor: {
          media_id: "maya-vtt-private",
          object_key: "private/maya-chen-v2.vtt",
          sha256: "9".repeat(64),
          mime_type: "text/vtt",
          size: 48122,
          original_filename: "maya-chen.vtt",
          format: "vtt",
          language: "en",
          version: 2,
        },
        transcript_segments: [
          {
            start: "00:02:14.000",
            end: "00:02:48.000",
            text: "Trust begins when sources remain visible.",
          },
          {
            start: "00:18:03.000",
            end: "00:18:31.000",
            text: "Make the process legible enough that audiences can inspect it.",
          },
        ],
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: "maya-interview-v2" },
      },
      [
        ["d", INTERVIEW_ID],
        ["version", "2"],
        ["status", "reviewing"],
        ["guest", GUEST_ID],
      ],
      now - 180,
    ),
    event(
      "4",
      30805,
      {
        schema_version: 2,
        record_type: "content",
        entity_id: CONTENT_ID,
        version: 1,
        status: "internal_review",
        title: "Maya Chen — trust is visible",
        interview_id: INTERVIEW_ID,
        publication_state: "not_published",
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: "maya-content-v1" },
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
        schema_version: 2,
        proposal_id: clipProposalId,
        proposal_version: 1,
        target_id: CONTENT_ID,
        target_kind: 30805,
        target_event_id: contentEventId,
        target_version: 1,
        agent: "Content / Clip Copilot",
        persona_id: "content-clip-copilot",
        run_id: "10000000-0000-4000-8000-000000000004",
        template_version: "timestamped-clips.v2",
        provider: "fixture",
        model: "deterministic-local-v1",
        input_event_ids: [interviewEventId, contentEventId],
        input_hash: "b".repeat(64),
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
        review_state: "pending",
      },
      [
        ["status", "proposed"],
        ["target", "30805", CONTENT_ID],
      ],
      now - 60,
      "c".repeat(64),
    ),
    event(
      "6",
      30809,
      {
        schema_version: 2,
        record_type: "approval",
        entity_id: "66666666-6666-4666-8666-666666666666",
        version: 1,
        status: "pending",
        title: "Partner review · Maya clip package",
        target_id: CONTENT_ID,
        target_kind: 30805,
        target_event_id: contentEventId,
        target_version: 1,
        proposal_id: clipProposalId,
        proposal_event_id: clipProposalEventId,
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: clipProposalEventId },
      },
      [
        ["d", "66666666-6666-4666-8666-666666666666"],
        ["version", "1"],
        ["status", "pending"],
        ["target", "30805", CONTENT_ID, contentEventId],
        ["proposal", clipProposalId, clipProposalEventId],
      ],
      now - 50,
    ),
    event(
      "7",
      30800,
      {
        schema_version: 2,
        record_type: "goal",
        entity_id: "70000000-0000-4000-8000-000000000000",
        version: 1,
        status: "active",
        title: "Build the most trusted interview desk",
        owner: "William",
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: "goal-1" },
      },
      [
        ["d", "70000000-0000-4000-8000-000000000000"],
        ["version", "1"],
        ["status", "active"],
      ],
      now - 45,
    ),
    event(
      "8",
      30801,
      {
        schema_version: 2,
        record_type: "project",
        entity_id: "80000000-0000-4000-8000-000000000000",
        version: 1,
        status: "blocked",
        title: "Maya Chen editorial package",
        blocked_reason: "Waiting for music-rights confirmation.",
        assignees: ["William"],
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: "project-1" },
      },
      [
        ["d", "80000000-0000-4000-8000-000000000000"],
        ["version", "1"],
        ["status", "blocked"],
      ],
      now - 40,
    ),
    event(
      "9",
      30802,
      {
        schema_version: 2,
        record_type: "task",
        entity_id: "90000000-0000-4000-8000-000000000000",
        version: 1,
        status: "in-progress",
        title: "Verify Maya transcript pull quotes",
        due_at: new Date((now + 18 * 3_600) * 1000).toISOString(),
        assignees: ["William"],
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: "task-1" },
      },
      [
        ["d", "90000000-0000-4000-8000-000000000000"],
        ["version", "1"],
        ["status", "in-progress"],
      ],
      now - 35,
    ),
    event(
      "a",
      30806,
      {
        schema_version: 2,
        record_type: "meeting",
        entity_id: "a0000000-0000-4000-8000-000000000000",
        version: 1,
        status: "planned",
        title: "Weekly editorial review",
        scheduled_at: new Date((now + 86_400) * 1000).toISOString(),
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: "meeting-1" },
      },
      [
        ["d", "a0000000-0000-4000-8000-000000000000"],
        ["version", "1"],
        ["status", "planned"],
      ],
      now - 30,
    ),
    event(
      "b",
      30807,
      {
        schema_version: 2,
        record_type: "decision",
        entity_id: "b0000000-0000-4000-8000-000000000000",
        version: 1,
        status: "decided",
        title: "Keep clips source-visible",
        rationale: "Editorial trust requires legible provenance.",
        source: "e2e-fixture",
        provenance: { type: "fixture", ref: "decision-1" },
      },
      [
        ["d", "b0000000-0000-4000-8000-000000000000"],
        ["version", "1"],
        ["status", "decided"],
      ],
      now - 28,
    ),
    event(
      "c1",
      48201,
      {
        schema_version: 2,
        proposal_id: "c1000000-0000-4000-8000-000000000000",
        proposal_version: 1,
        run_id: "10000000-0000-4000-8000-000000000002",
        target: {
          kind: 30803,
          id: GUEST_ID,
          event_id: guestEventId,
          version: 1,
        },
        agent: "Outreach Drafter",
        persona_id: "outreach-drafter",
        proposal_type: "outreach_draft",
        template_version: "outreach-draft.v2",
        provider: "fixture",
        model: "deterministic-local-v1",
        input_event_ids: [guestEventId],
        input_hash: "c".repeat(64),
        summary: "Personalized email draft; no delivery was attempted.",
        provenance: ["Reviewed Maya research"],
        output: {
          subject: "A conversation about trust people can inspect",
          body: "Hi Maya — your work on visible sourcing raises exactly the kind of practical question MK Ideas explores. We would love to invite you for a conversation. This remains an unsent draft.",
          delivery_state: "not_sent",
          dnc_checked: true,
        },
        status: "proposed",
        review_state: "pending",
      },
      [
        ["status", "proposed"],
        ["target", "30803", GUEST_ID],
      ],
      now - 25,
      "d".repeat(64),
    ),
    event(
      "c2",
      48201,
      {
        schema_version: 2,
        proposal_id: "c2000000-0000-4000-8000-000000000000",
        proposal_version: 1,
        run_id: "10000000-0000-4000-8000-000000000003",
        target: {
          kind: 30804,
          id: INTERVIEW_ID,
          event_id: interviewEventId,
          version: 2,
        },
        agent: "Interview Producer",
        persona_id: "interview-producer",
        proposal_type: "interview_brief",
        template_version: "interview-brief.v2",
        provider: "fixture",
        model: "deterministic-local-v1",
        input_event_ids: [guestEventId, interviewEventId],
        input_hash: "d".repeat(64),
        summary: "A source-grounded interview brief and question set is ready.",
        provenance: ["Guest profile v1", "Interview record v2"],
        output: {
          questions: [
            "What makes sourcing feel trustworthy?",
            "Where must human judgment stay visible?",
          ],
        },
        status: "proposed",
        review_state: "pending",
      },
      [
        ["status", "proposed"],
        ["target", "30804", INTERVIEW_ID],
      ],
      now - 20,
      "e".repeat(64),
    ),
    event(
      "d1",
      48203,
      {
        schema_version: 2,
        summary_id: "d1000000-0000-4000-8000-000000000000",
        summary_type: "operations_briefing",
        persona_id: "operations-briefing-assistant",
        status: "informational",
        provenance: ["Current MK heads"],
        output: {
          approvals: ["One clip package awaits partner review."],
          deadlines: ["Transcript pull quotes are due today."],
          blocked_work: ["The Maya package is waiting on rights confirmation."],
          recommendations: ["Review the timestamped clip proposal next."],
        },
      },
      [],
      now - 15,
      "f".repeat(64),
    ),
    event(
      "e1",
      48204,
      {
        schema_version: 2,
        activity_type: "mention",
        target: { kind: 30802, id: "90000000-0000-4000-8000-000000000000" },
        persona_id: "",
        status: "open",
        summary: "A partner mentioned you in the transcript verification task.",
      },
      [],
      now - 10,
    ),
    event(
      "e2",
      48204,
      {
        schema_version: 2,
        activity_type: "agent_run",
        run_id: "10000000-0000-4000-8000-000000000005",
        persona_id: "operations-briefing-assistant",
        status: "succeeded",
        summary: "The daily operating brief completed.",
      },
      [],
      now - 5,
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

test("MK Ideas workspace is coherent across all five permanent areas", async ({
  page,
}) => {
  mkdirSync(OUTPUT, { recursive: true });
  await page.setViewportSize({ width: 1440, height: 900 });

  await page.goto("/#/people");
  await expect(page.getByTestId("mkideas-people")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Maya Chen", exact: true }),
  ).toBeVisible();
  await expect(page.getByText("Relationship profile")).toBeVisible();
  await expect(page.getByText("Research & outreach drafts")).toBeVisible();
  await expect(page.getByText(/Sending disabled/i)).toBeVisible();
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/07-people-profile.png` });
  const peopleAgents = page.getByRole("region", {
    name: "Contextual MK Ideas agents",
  });
  await peopleAgents.scrollIntoViewIfNeeded();
  await expect(
    page.getByText("Guest Researcher", { exact: true }).last(),
  ).toBeVisible();
  await expect(
    page.getByText("Outreach Drafter", { exact: true }).last(),
  ).toBeVisible();
  await waitForAnimations(page);
  await peopleAgents.screenshot({ path: `${OUTPUT}/08-people-agents.png` });

  await page.goto("/#/");
  await expect(page.getByTestId("mkideas-today")).toBeVisible();
  await expect(page.getByText("Operational inbox")).toBeVisible();
  await expect(page.getByText("Ranked and deduplicated")).toBeVisible();
  await expect(page.getByText("Daily brief")).toBeVisible();
  await expect(
    page.getByText(/One clip package awaits partner review/),
  ).toBeVisible();
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/09-today-ranked-inbox.png` });

  await page.goto("/#/studio");
  await expect(page.getByTestId("mkideas-studio")).toBeVisible();
  await expect(
    page.getByRole("heading", {
      name: "Interview with Maya Chen",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText("Trust begins with visible sources"),
  ).toBeVisible();
  await expect(page.getByText("Authorized transcript index")).toBeVisible();
  await expect(
    page.getByText(/Full transcript bytes stay outside/),
  ).toBeVisible();
  const studioDetail = page.getByTestId("mkideas-studio-record-detail");
  await waitForAnimations(page);
  await studioDetail.screenshot({
    path: `${OUTPUT}/10-studio-transcript-descriptor.png`,
  });
  await expect(page.getByRole("button", { name: "Approve" })).toBeVisible();
  await page
    .getByLabel("Review reason")
    .fill(
      "The timestamps and captions match the transcript and editorial framing.",
    );
  const approvalCountBefore = await page.evaluate(
    () =>
      window.__BUZZ_E2E_MKIDEAS_EVENTS__?.filter(
        (event) => event.kind === 30809,
      ).length ?? 0,
  );
  await page.getByRole("button", { name: "Approve" }).click();
  await expect(page.getByText("Human review · approved")).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__BUZZ_E2E_MKIDEAS_EVENTS__?.filter(
            (event) => event.kind === 48200,
          ).length ?? 0,
      ),
    )
    .toBe(1);
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__BUZZ_E2E_MKIDEAS_EVENTS__?.filter(
            (event) => event.kind === 30809,
          ).length ?? 0,
      ),
    )
    .toBe(approvalCountBefore);
  const studioAgents = page.getByRole("region", {
    name: "Contextual MK Ideas agents",
  });
  await studioAgents.scrollIntoViewIfNeeded();
  await expect(
    page.getByText("Interview Producer", { exact: true }).last(),
  ).toBeVisible();
  await expect(
    page.getByText("Content / Clip Copilot", { exact: true }).last(),
  ).toBeVisible();
  await waitForAnimations(page);
  await studioAgents.screenshot({ path: `${OUTPUT}/11-studio-agents.png` });

  await page.goto("/#/");
  const todayAgents = page.getByRole("region", {
    name: "Contextual MK Ideas agents",
  });
  await todayAgents.scrollIntoViewIfNeeded();
  await expect(
    page.getByText("Operations Briefing Assistant", { exact: true }).last(),
  ).toBeVisible();
  await waitForAnimations(page);
  await todayAgents.screenshot({ path: `${OUTPUT}/12-today-agent.png` });

  await expect(page.getByTestId("open-work-view")).toBeVisible();
  await expect(page.getByTestId("open-people-view")).toBeVisible();
  await expect(page.getByTestId("open-studio-view")).toBeVisible();
  await expect(page.getByTestId("open-team-view")).toBeVisible();

  await page.getByTestId("open-work-view").click();
  await expect(page.getByTestId("mkideas-work")).toBeVisible();
  await expect(
    page.getByRole("navigation", { name: "Work record types" }),
  ).toBeVisible();
  await page.getByTestId("mkideas-quick-capture").click();
  await expect(page.getByRole("button", { name: "Guest" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Task", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("button", { name: "Meeting" })).toBeVisible();
  const capturedTask = "Confirm Maya interview brief";
  await page.getByLabel("Title").fill(capturedTask);
  await page.getByRole("button", { name: "Save human-signed task" }).click();
  await expect
    .poll(() =>
      page.evaluate(
        (title) =>
          window.__BUZZ_E2E_MKIDEAS_EVENTS__?.some(
            (stored) => stored.kind === 30802 && stored.content.includes(title),
          ) ?? false,
        capturedTask,
      ),
    )
    .toBe(true);
  await expect(page.getByText(capturedTask, { exact: true })).toBeVisible();
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/04-work-quick-capture.png` });

  await page.getByTestId("channel-general").click();
  await waitForMockLiveSubscription(page, "general");
  const recordLink = `buzz://mkideas/entity?community=${HOST}&kind=30805&d=${CONTENT_ID}&proposal=55555555-5555-4555-8555-555555555555`;
  await page.evaluate((link) => {
    window.__BUZZ_E2E_EMIT_MOCK_MESSAGE__?.({
      channelName: "general",
      content: `Review discussion for the Maya Chen clip proposal\n[Content record](${link})`,
    });
  }, recordLink);
  await expect(
    page.getByText("Review discussion for the Maya Chen clip proposal"),
  ).toBeVisible();
  const teamRecordLink = page.getByRole("button", {
    name: "Open MK Ideas content 33333333",
  });
  await expect(teamRecordLink).toBeVisible();
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/05-team-linked-discussion.png` });
  await teamRecordLink.click();
  await expect(page.getByTestId("mkideas-studio")).toBeVisible();

  await page.getByTestId("open-search").click();
  await page.getByTestId("search-dialog-input").fill(capturedTask);
  const mkResult = page
    .locator('[data-search-section="mkideas"] .search-result-row')
    .filter({ hasText: capturedTask });
  await expect(mkResult).toHaveCount(1);
  await expect(mkResult).toContainText("Task · to-do · v1");
  await expect(mkResult).toContainText("work");
  await waitForAnimations(page);
  await page.screenshot({ path: `${OUTPUT}/06-typed-universal-search.png` });
  await mkResult.click();
  await expect(page.getByTestId("mkideas-work")).toBeVisible();
  await expect(page.getByText(capturedTask, { exact: true })).toBeVisible();
});
