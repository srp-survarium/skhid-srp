package survarium.constants
{
   // A quest's status (the exe passes 2 and 3 to 0x28E690).
   public class QuestStatus
   {
      
      // meaning not proven: QuestWindow treats it as a quest on offer
      public static const OFFERED:int = 0;
      
      // meaning unknown: only the movie's test data uses it
      public static const STATUS_1:int = 1;
      
      public static const COMPLETED:int = 2;
      
      public static const FAILED:int = 3;
      
      public function QuestStatus()
      {
         super();
      }
   }
}
