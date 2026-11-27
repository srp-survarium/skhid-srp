package survarium.upgrade
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.events.ButtonEvent;
   import survarium.data.LocalizationData;

   // The reticle of the collimator in a weapon's scope slot (UpgradeWindow, under the slot's label):
   // a title and four steppers, one row each (ReticleChoice.STEPPERS: pattern, size, colour,
   // brightness), each its name, a back arrow, its value and a next arrow (PageArrow at the rows'
   // size, the window's Confirm art). An arrow whose step changes nothing (the brightness at its end,
   // a list of one) is disabled. A press dispatches STEP with the choice one step on (`stepped`); the
   // panel shows only what it is given (show), the account's choice.
   public class ReticlePanel extends Sprite
   {

      public static const STEP:String = "reticleStep";

      public static const WIDTH:Number = 180;

      protected static const TITLE_HEIGHT:Number = 18;

      protected static const ROW_HEIGHT:Number = 20;

      protected static const ROW_GAP:Number = 3;

      public static const HEIGHT:Number = TITLE_HEIGHT + 4 * ROW_HEIGHT + 3 * ROW_GAP;

      // a row: its name LABEL_WIDTH wide, then the back arrow, the value and the next arrow, GAP apart
      protected static const LABEL_WIDTH:Number = 60;

      protected static const ARROW_WIDTH:Number = 16;

      protected static const GAP:Number = 2;

      protected static const VALUE_WIDTH:Number = WIDTH - LABEL_WIDTH - 2 * ARROW_WIDTH - 3 * GAP;

      protected static const TEXT_SIZE:int = 13;

      // the smallest size a long text is shrunk to (UpgradeWindow.fitText's)
      protected static const TEXT_MIN_SIZE:int = 9;

      // SideButton's colours: names grey, values light
      protected static const NAME_COLOUR:uint = 0xB4BEC4;

      protected static const VALUE_COLOUR:uint = 0xE7F1EE;

      public var title:TextField;

      // by stepper (ReticleChoice.STEPPERS)
      public var labels:Array = [];

      public var values:Array = [];

      public var backArrows:Array = [];

      public var nextArrows:Array = [];

      // the choice a press steps from (show's), null none
      protected var _choice:ReticleChoice = null;

      // the choice the last press asks for (STEP's)
      public var stepped:ReticleChoice = null;

      public function ReticlePanel()
      {
         super();
         var stepper:int = 0;
         var y:Number = TITLE_HEIGHT;
         this.title = this.addText("reticleTitle",0,0,WIDTH,TITLE_HEIGHT,"center");
         for each(stepper in ReticleChoice.STEPPERS)
         {
            this.labels[stepper] = this.addText("reticleLabel" + stepper,0,y,LABEL_WIDTH,ROW_HEIGHT,"left");
            this.backArrows[stepper] = this.addArrow("reticleBack" + stepper,LABEL_WIDTH + GAP,y,false);
            this.values[stepper] = this.addText("reticleValue" + stepper,LABEL_WIDTH + 2 * GAP + ARROW_WIDTH,y,VALUE_WIDTH,ROW_HEIGHT,"center");
            this.nextArrows[stepper] = this.addArrow("reticleNext" + stepper,WIDTH - ARROW_WIDTH,y,true);
            y += ROW_HEIGHT + ROW_GAP;
         }
      }

      // The choice shown (its texts in the current language, the arrows that step it enabled); null
      // hides the panel.
      public function show(choice:ReticleChoice) : void
      {
         var stepper:int = 0;
         this._choice = choice;
         visible = choice != null;
         if(choice == null)
         {
            return;
         }
         fit(this.title,LocalizationData.getName("st_srp_reticle_title"),VALUE_COLOUR);
         for each(stepper in ReticleChoice.STEPPERS)
         {
            fit(this.labels[stepper],LocalizationData.getName(ReticleChoice.labelKey(stepper)),NAME_COLOUR);
            fit(this.values[stepper],choice.text(stepper),VALUE_COLOUR);
            PageArrow(this.backArrows[stepper]).enabled = choice.canStep(stepper,-1);
            PageArrow(this.nextArrows[stepper]).enabled = choice.canStep(stepper,1);
         }
      }

      public function get choice() : ReticleChoice
      {
         return this._choice;
      }

      protected function addText(name:String, x:Number, y:Number, w:Number, h:Number, align:String) : TextField
      {
         var field:TextField = new TextField();
         field.name = name;
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.multiline = false;
         field.wordWrap = false;
         field.autoSize = TextFieldAutoSize.NONE;
         field.x = x;
         field.y = y;
         field.width = w;
         field.height = h;
         field.defaultTextFormat = format(TEXT_SIZE,NAME_COLOUR,align);
         addChild(field);
         return field;
      }

      protected function addArrow(name:String, x:Number, y:Number, pointRight:Boolean) : PageArrow
      {
         var arrow:PageArrow = new PageArrow();
         arrow.name = name;
         arrow.pointRight = pointRight;
         arrow.resizeArrow(ARROW_WIDTH,ROW_HEIGHT);
         arrow.x = x;
         arrow.y = y;
         arrow.addEventListener(ButtonEvent.CLICK,this.onArrow);
         addChild(arrow);
         return arrow;
      }

      protected static function format(size:int, colour:uint, align:String) : TextFormat
      {
         var f:TextFormat = new TextFormat("Calibri",size,colour,true);
         f.align = align;
         return f;
      }

      // The text on one line in its field, its size cut until it fits.
      protected static function fit(field:TextField, text:String, colour:uint) : void
      {
         var size:int = TEXT_SIZE;
         var align:String = field.defaultTextFormat.align;
         field.text = text;
         while(true)
         {
            field.defaultTextFormat = format(size,colour,align);
            field.setTextFormat(field.defaultTextFormat);
            if(field.textWidth + 4 <= field.width && field.numLines <= 1 || size <= TEXT_MIN_SIZE)
            {
               break;
            }
            size--;
         }
      }

      private function onArrow(event:ButtonEvent) : void
      {
         var arrow:PageArrow = event.currentTarget as PageArrow;
         var stepper:int = this.nextArrows.indexOf(arrow);
         var direction:int = 1;
         if(stepper < 0)
         {
            stepper = this.backArrows.indexOf(arrow);
            direction = -1;
         }
         if(stepper < 0 || this._choice == null || !arrow.enabled || !this._choice.canStep(stepper,direction))
         {
            return;
         }
         this.stepped = this._choice.stepped(stepper,direction);
         dispatchEvent(new Event(STEP));
      }
   }
}
