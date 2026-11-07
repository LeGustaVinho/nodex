using System.Collections;
using UnityEngine;

namespace Game
{
    public class PlayerController : MonoBehaviour
    {
        [SerializeField] private float speed = 5f;
        [SerializeField] private Rigidbody body;

        private int _jumps;

        private void Awake()
        {
            body = GetComponent<Rigidbody>();
        }

        private void Update()
        {
            float horizontal = Input.GetAxis("Horizontal");
            transform.Translate(Vector3.right * horizontal * speed * Time.deltaTime);

            if (Input.GetKeyDown(KeyCode.Space) && _jumps < 2)
            {
                body.AddForce(Vector3.up * 5f, ForceMode.Impulse);
                _jumps++;
            }
        }

        private void OnCollisionEnter(Collision collision)
        {
            if (collision.gameObject.CompareTag("Ground"))
            {
                _jumps = 0;
                StartCoroutine(Flash());
            }
        }

        private IEnumerator Flash()
        {
            var renderer = GetComponent<Renderer>();
            for (int i = 0; i < 3; i++)
            {
                renderer.enabled = false;
                yield return new WaitForSeconds(0.1f);
                renderer.enabled = true;
                yield return new WaitForSeconds(0.1f);
            }
        }
    }
}
