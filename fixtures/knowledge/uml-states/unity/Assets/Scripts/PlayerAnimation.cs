using UnityEngine;

public class PlayerAnimation : MonoBehaviour
{
    [SerializeField] private Animator animator;

    void Update()
    {
        animator.SetBool("Grounded", true);
        if (Input.GetButtonDown("Jump"))
        {
            Jump();
        }
    }

    void Jump()
    {
        animator.SetTrigger("Jump");
    }

    public void Die()
    {
        animator.Play("Death");
    }

    public void Respawn()
    {
        animator.CrossFade("Idle", 0.2f);
    }
}
