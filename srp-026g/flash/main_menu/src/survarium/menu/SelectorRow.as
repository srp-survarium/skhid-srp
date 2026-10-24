package survarium.menu
{
   import flash.display.MovieClip;

   // An options row the timeline does not place: the game's arrow selector (`< value >`, the
   // OptionStepperCyclic of the video options: its label on the left, the value between two
   // arrows), made at its place in its tab. What it shows and sets is the exe's option of that
   // row: its label, its values' labels and the saved variable the choice sets (exe patch 64's
   // interface settings: interface_026g.c's table, the gameplay options' rows after the game's
   // 16), bound by Settings.create_content as the timeline's own controls are.
   //
   // A new setting: a row of interface_026g.c's table (its variable, label and values' keys,
   // what the match's movies get), a row of ROWS here in the same order, and its labels in a
   // content edit (docs/notes/interface-level.md).
   public class SelectorRow
   {
      // {tab: the options' tab (0 the gameplay one), name: the control's name, after the tab's
      // own in Settings' element map, x, y: its place in the tab}
      public static const ROWS:Array = [{
         tab:0,
         name:"_interface_level",
         x:322,
         y:285
      }];

      public function SelectorRow()
      {
         super();
      }

      // The names of a tab's rows, in the order of the exe's (Settings.configUI).
      public static function names(tab:uint) : Array
      {
         var out:Array = [];
         for each(var row:Object in ROWS)
         {
            if(row.tab == tab)
            {
               out.push(row.name);
            }
         }
         return out;
      }

      // Before the shown tab's controls are bound: each row made once (a child made by script stays
      // on the timeline's other frames), hidden until show (create_content returns early while the
      // tab's settings have not come, when a row would show no label).
      public static function place(content:MovieClip) : void
      {
         var control:OptionStepperCyclic;
         for each(var row:Object in ROWS)
         {
            control = content[row.name] as OptionStepperCyclic;
            if(control == null)
            {
               control = new OptionStepperCyclic();
               control.name = row.name;
               control.x = row.x;
               control.y = row.y;
               content[row.name] = control;
               content.addChild(control);
            }
            control.visible = false;
         }
      }

      // The shown tab's controls bound: its rows shown.
      public static function show(content:MovieClip) : void
      {
         var control:OptionStepperCyclic;
         for each(var row:Object in ROWS)
         {
            control = content[row.name] as OptionStepperCyclic;
            if(control != null && content.currentFrame == row.tab + 1)
            {
               control.visible = true;
               // Scaleform leaves a control made by script fully transparent (as SrpFovSlider's slider)
               control.alpha = 1;
            }
         }
      }
   }
}
