# RFCs - OpenDAL Design History

RFCs power OpenDAL's development and preserve the history behind substantial
design decisions.

An accepted RFC is an immutable historical document. It records the proposal
that reached consensus at the time of acceptance, but it is not the current
contract. [OpenDAL specifications][crate::docs::specs] define the living
contracts implemented by current releases.

The "RFC" (request for comments) process is intended to provide a consistent and controlled path for changes to OpenDAL (such as new features) so that all stakeholders can be confident about the direction of the project.

Many changes, including bug fixes and documentation improvements, can be implemented and reviewed via the normal GitHub pull request workflow.

Some changes, though, are "substantial" and we ask that these be put through a bit of a design process and produce a consensus among the OpenDAL community.

### Which kinds of changes require an RFC?

Any substantial change or addition to the project that would require a significant amount of work to implement should generally be an RFC. 

Some examples include:

- A new feature that creates a new public API or raw API. 
- The removal of features that already shipped as part of the release.
- A big refactor of existing code or reorganization of code into new modules.

Those are just a few examples. Ultimately, the judgment call of what constitutes a big enough change to warrant an RFC is left to the project maintainers.

If you submit a pull request to implement a new feature without going through the RFC process, it may be closed with a polite request to submit an RFC first.

## Before creating the RFC

Preparing in advance before submitting an RFC hastily can increase its chances of being accepted. If you have proposals to make, it is advisable to engage in some preliminary groundwork to facilitate a smoother process.

It is great to seek feedback from other project developers first, as this can help validate the viability of the RFC. To ensure a sustained impact on the project, it is important to work together and reach a consensus.

Common preparatory steps include presenting your idea on platforms such as GitHub [issues](https://github.com/apache/opendal/issues/) or [discussions](https://github.com/apache/opendal/discussions/categories/ideas), or engaging in discussions through our [email list](https://opendal.apache.org/community/#mailing-list) or [Discord server](https://opendal.apache.org/discord). 

## The RFC process

- Fork the [OpenDAL repo](https://github.com/apache/opendal) and create your branch from `main`.
- Copy [`0000_example.md`] to `0000-my-feature.md` (where "my-feature" is descriptive). Don't assign an RFC number yet; This is going to be the PR number, and we'll rename the file accordingly if the RFC is accepted.
- Submit a pull request. As a pull request, the RFC will receive design feedback from the larger community, and the author should be prepared to revise it in response.
- Now that your RFC has an open pull request, use the issue number of this PR to update your `0000-` prefix to that number.
- Build consensus and integrate feedback. RFCs that have broad support are much more likely to make progress than those that don't receive any comments. Feel free to reach OpenDAL maintainers for help.
- RFCs rarely go through this process unchanged, especially as alternatives and drawbacks are shown. You can make edits, big and small, to the RFC to clarify or change the design, but make changes as new commits to the pull request, and leave a comment on the pull request explaining your changes. Specifically, do not squash or rebase commits after they are visible on the pull request.
- The RFC pull request lasts for three days after the last update. After that, the RFC will be accepted or declined based on the consensus reached in the discussion.
- For the accepting of an RFC, we will require approval from at least three maintainers.
- Once the RFC is accepted, please create a tracking issue and update links in RFC. And then the PR will be merged and the RFC will become 'active' status.

## Implementing an RFC

An active RFC does not indicate the priority assigned to its implementation,
nor does it imply that a developer has been specifically assigned the task of implementing the feature.

The RFC author is encouraged to submit an implementation after the RFC has been accepted.
Nevertheless, it is not obligatory for them to do so.

Accepted RFCs may represent features that can wait until a developer chooses to work on them.
Each accepted RFC is associated with an issue in the OpenDAL repository, which tracks its implementation.

If you are interested in implementing an RFC but are unsure if someone else is already working on it,
feel free to inquire by leaving a comment on the associated issue.

After implementation, update or add a specification when the RFC establishes
portable user-visible behavior. Later contract changes update the specification
and, when substantial, require a new RFC. They do not rewrite the accepted RFC.

## Some useful tips

- The author of an RFC may not be the same one as the implementer. Therefore, when submitting an RFC, it is advisable to include sufficient information.
- Do not modify an accepted RFC. Correct the current contract in a specification
  and create a new RFC when the change is substantial.
