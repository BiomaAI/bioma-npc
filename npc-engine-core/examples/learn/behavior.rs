use bioma_npc_core::{Behavior, Context, IdleTask, Task};

use crate::{
    domain::LearnDomain,
    task::{collect::Collect, left::Left, right::Right},
};

pub struct DefaultBehaviour;
impl Behavior<LearnDomain> for DefaultBehaviour {
    fn add_own_tasks(
        &self,
        ctx: Context<LearnDomain>,
        tasks: &mut Vec<Box<dyn Task<LearnDomain>>>,
    ) {
        tasks.push(Box::new(IdleTask));
        let possible_tasks: [Box<dyn Task<LearnDomain>>; 3] =
            [Box::new(Collect), Box::new(Left), Box::new(Right)];
        for task in &possible_tasks {
            if task.is_valid(ctx) {
                tasks.push(task.clone());
            }
        }
    }

    fn is_valid(&self, _ctx: Context<LearnDomain>) -> bool {
        true
    }
}
