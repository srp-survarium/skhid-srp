package survarium.constants
{
   // Key codes the movies get that flash.ui.Keyboard has no name for.
   public class KeyCode
   {
      
      // meaning unknown: the chats submit on it, but no key of the 0.26g exe's bind table gives it
      public static const CHAT_SUBMIT:int = 150;
      
      // Windows VK_LSHIFT (the exe maps DIK_LSHIFT to it, 0x1FE91F)
      public static const LEFT_SHIFT:int = 160;
      
      public function KeyCode()
      {
         super();
      }
   }
}
