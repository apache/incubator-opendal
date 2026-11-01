---
title: Nominate PMC Member
sidebar_position: 3
---

This document mainly introduces how a PMC member nominates a new PMC member.

## Start vote about the candidate

Start a vote about the candidate via sending email to: <private@opendal.apache.org>:

- candidate_name: The full name of the candidate.
- candidate_github_id: The GitHub id of the candidate.

Title:

```
[VOTE] Add candidate ${candidate_name} as a new PMC member
```

Content:

```
Hi, All OpenDAL PMC members.
  
I would like to nominate ${candidate_name} (GitHub id: ${candidate_github_id}) as a candidate for the OpenDAL PMC member. Since becoming an OpenDAL committer, ${candidate_name} has made significant contributions to various modules of the project.

${candidate_name}'s great contributions can be found:

- GitHub Account: https://github.com/${candidate_github_id}
- GitHub Pull Requests: https://github.com/apache/opendal/pulls?q=is%3Apr+author%3A${candidate_github_id}
- GitHub Issues: https://github.com/apache/opendal/issues?q=is%3Aissue+involves%3A${candidate_github_id}

Please make your valuable evaluation on whether we could invite ${candidate_name} as a
PMC member:

[ +1 ] Agree to add ${candidate_name} as a PMC member of OpenDAL.
[ 0  ] Have no sense.
[ -1 ] Disagree to add ${candidate_name} as a PMC member of OpenDAL, because .....

This vote starts from the moment of sending and will be open for 3 days.
 
Thanks and best regards,

${your_name}
```

Example: <https://lists.apache.org/thread/yg2gz2tof3cvbrgp1wxzk6mf9o858h7t> (Private Link)

After at least 3 `+1` binding vote and no veto, claim the vote result:

Title:

```
[RESULT][VOTE] Add candidate ${candidate_name} as a new PMC member
```

Content:

```
Hi, all:

The vote for "Add candidate ${candidate_name} as a new PMC member" has PASSED and closed now.

The result is as follows:

4 binding +1 Votes:
- voter names

Vote thread: https://lists.apache.org/thread/yg2gz2tof3cvbrgp1wxzk6mf9o858h7t

Then I'm going to invite ${candidate_name} to join us.

Thanks for everyone's support!

${your_name}
```

## Send invitation to the candidate

Send an invitation to the candidate and cc <private@opendal.apache.org>:

Title:

```
Invitation to become Apache OpenDAL PMC Member: ${candidate_name}
```

Content:

```
Hello ${candidate_name},

In recognition of your contributions to Apache OpenDAL, the OpenDAL PMC has recently voted to add you as a PMC member. The role of a PMC member grants you access to the Project Management Committee (PMC) and enables you to take on greater responsibilities within the OpenDAL project. We hope that you accept this invitation and continue to help us make Apache OpenDAL better.

Please reply to private@opendal.apache.org using the 'reply all' function for your responses.

With the expectation of your acceptance, welcome!

${your_name} (as represents of The Apache OpenDAL PMC)
```

## Add the candidate to the PMC member list

After the candidate accepts the invitation, add the candidate to the PMC member list by [whimsy roster tools](https://whimsy.apache.org/roster/committee/opendal)

![](roster-add-pmc-member.png)
