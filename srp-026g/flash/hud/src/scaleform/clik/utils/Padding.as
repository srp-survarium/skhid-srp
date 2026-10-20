package scaleform.clik.utils
{
   public class Padding
   {
      
      public var top:Number = 0;
      
      public var bottom:Number = 0;
      
      public var left:Number = 0;
      
      public var right:Number = 0;
      
      public function Padding(... args)
      {
         super();
         switch(args.length)
         {
            case 0:
               break;
            case 1:
               this.top = this.right = this.bottom = this.left = Number(args[0]);
               break;
            case 2:
               this.top = this.bottom = Number(args[0]);
               this.right = this.left = Number(args[1]);
               break;
            case 4:
               this.top = Number(args[0]);
               this.right = Number(args[1]);
               this.bottom = Number(args[2]);
               this.left = Number(args[3]);
               break;
            default:
               throw new Error("Padding can not have " + args.length + " values");
         }
      }
      
      public function get vertical() : Number
      {
         return this.top + this.bottom;
      }
      
      public function get horizontal() : Number
      {
         return this.left + this.right;
      }
      
      public function toString() : String
      {
         return "[Padding top=" + this.top + " bottom=" + this.bottom + " left=" + this.left + " right=" + this.right + "]";
      }
   }
}

